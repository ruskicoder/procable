@echo off
setlocal
rem Procable launcher (Windows). Runs bin\windows-<arch>\procable.exe next
rem to this file, else a cached or freshly downloaded engine binary.
rem
rem Structure notes (this file is exercised by dry parsing and string-level
rem tests, not yet on a real Windows machine):
rem - No multi-line parenthesized blocks: cmd expands %var% at block parse
rem   time, which made the old download path read back empty %url%/%cached%.
rem   Linear goto flow keeps every expansion on its own line, and avoids
rem   delayed expansion eating ! characters in user arguments.
rem - The unversioned user binary and the PATH candidate are validated with
rem   the engine-probe handshake (see :probe) so the retired 3.x npm CLI,
rem   whose bin is also named procable, is never exec'd. The sibling binary
rem   must also report the version in VERSION: it is not tracked, so a skill
rem   upgrade can leave an old engine in place. PROCABLE_BIN stays trusted.
rem - Downloads and the version-pinned cache are verified via certutil
rem   against engine-checksums.txt next to this file, and fail closed; a
rem   .verified stamp lets later runs skip hashing the cache while it holds
rem   the manifest digest and the binary's size and is not older than the
rem   binary. On ARM64 the arm64 asset is tried first and the x64 asset is the
rem   fallback (Windows on ARM runs x64 binaries), each only when the manifest
rem   has a line for it.
if not defined PROCABLE_SKILL_DIR set "PROCABLE_SKILL_DIR=%~dp0.."
if not defined PROCABLE_SELF set "PROCABLE_SELF=%~f0"
set "arch=x64"
if /I "%PROCESSOR_ARCHITECTURE%"=="ARM64" set "arch=arm64"

if not defined PROCABLE_BIN goto no_env_bin
if not exist "%PROCABLE_BIN%" goto no_env_bin
set "run=%PROCABLE_BIN%"
goto run
:no_env_bin

set "version="
if exist "%~dp0VERSION" set /p version=<"%~dp0VERSION"
set "bin=%~dp0bin\windows-%arch%\procable.exe"
if not exist "%bin%" goto no_sibling
if defined PROCABLE_LAUNCHER_PROBE goto sibling_ok
if not defined version goto sibling_ok
call :probe "%bin%"
if "%probe_version%"=="%version%" goto sibling_ok
if not defined probe_version set "probe_version=unknown"
echo procable: skipping "%bin%": it reports engine %probe_version% but this skill pins %version%; delete it to stop this warning 1>&2
goto no_sibling
:sibling_ok
set "run=%bin%"
goto run
:no_sibling

set "home_bin=%USERPROFILE%\.procable\bin\procable.exe"
if not exist "%home_bin%" goto no_home_bin
if defined PROCABLE_LAUNCHER_PROBE goto no_home_bin
call :probe "%home_bin%"
if not "%probe_ok%"=="1" goto no_home_bin
set "run=%home_bin%"
goto run
:no_home_bin

if not defined PROCABLE_HOME set "PROCABLE_HOME=%USERPROFILE%\.procable"
set "cached=%PROCABLE_HOME%\bin\%version%\procable.exe"
set "asset=procable-windows-%arch%.exe"
set "manifest=%~dp0engine-checksums.txt"
if not defined version goto no_cache
if not exist "%cached%" goto no_cache
call :read_expected
for %%f in ("%cached%") do set "size=%%~zf"
set "stamp="
if exist "%cached%.verified" for /f "usebackq tokens=1,2" %%s in ("%cached%.verified") do set "stamp=%%s"& set "stamp_size=%%t"
if not defined stamp goto cache_hash
if not "%stamp_size%"=="%size%" goto cache_hash
if /I "%stamp%"=="%expected%" goto cache_stamp_matches
if /I "%stamp%"=="%expected_x64%" goto cache_stamp_matches
goto cache_hash
:cache_stamp_matches
rem Newest first: the stamp must be listed before the binary it vouches for.
set "newest="
for /f "delims=" %%f in ('dir /b /a:-d /o:-d "%cached%*" 2^>nul') do if not defined newest set "newest=%%f"
if /I "%newest%"=="procable.exe.verified" goto cache_trusted
:cache_hash
call :sha256 "%cached%" "%TEMP%\procable-hash-%RANDOM%%RANDOM%.txt"
if not defined actual goto cache_unverified
if /I "%actual%"=="%expected%" goto cache_verified
if /I "%actual%"=="%expected_x64%" goto cache_verified
:cache_unverified
echo procable: cannot verify the cached engine "%cached%" against "%manifest%"; not using it 1>&2
goto no_cache
:cache_verified
(>"%cached%.verified" echo %actual% %size%) 2>nul
:cache_trusted
set "run=%cached%"
goto run
:no_cache

if defined PROCABLE_LAUNCHER_PROBE goto download
where procable >nul 2>nul
if errorlevel 1 goto download
call :probe procable
if not "%probe_ok%"=="1" goto download
procable %*
exit /b %errorlevel%

:download
rem procable runs offline: the launcher never downloads an engine. The
rem prompt-orchestrator installer ships it at %bin%; PROCABLE_BIN or a
rem binary on PATH also work.
if defined PROCABLE_LAUNCHER_PROBE exit /b 127
goto fail

:read_expected
rem Sets expected (and, on ARM64, expected_x64: Windows on ARM runs x64
rem binaries) from engine-checksums.txt.
call :manifest_digest "%asset%" expected
set "expected_x64="
if "%arch%"=="arm64" call :manifest_digest procable-windows-x64.exe expected_x64
exit /b 0

:manifest_digest
rem Sets the variable named %2 to the digest the first engine-checksums.txt
rem line for this version and asset %1 records, empty when there is none.
set "%~2="
if not exist "%manifest%" exit /b 0
for /f "usebackq tokens=1-3" %%a in ("%manifest%") do if "%%a"=="%version%" if /I "%%b"=="%~1" if not defined %~2 set "%~2=%%c"
exit /b 0

:sha256
rem Sets actual to the sha256 of %~1, empty when certutil fails. %~2 is the
rem scratch file for its output. Check certutil's status before parsing: its
rem error text on stdout is not a digest.
set "actual="
certutil -hashfile "%~1" SHA256 >"%~2" 2>nul
if errorlevel 1 goto sha256_done
for /f "usebackq skip=1 delims=" %%h in ("%~2") do if not defined actual set "actual=%%h"
if not defined actual goto sha256_done
set "actual=%actual: =%"
:sha256_done
del "%~2" >nul 2>nul
exit /b 0

:run
"%run%" %*
exit /b %errorlevel%

:probe
rem Sets probe_ok=1 when %1 answers the engine handshake: prints
rem "procable-engine <version>" and exits 0, and probe_version to that
rem version. The 3.x npm CLI answers any unknown verb with "Unknown command",
rem exit 1, so it never passes.
set "probe_ok="
set "probe_version="
set "probe_tmp=%TEMP%\procable-probe-%RANDOM%%RANDOM%.txt"
set "PROCABLE_LAUNCHER_PROBE=1"
"%~1" engine-probe >"%probe_tmp%" 2>nul
set "probe_err=%ERRORLEVEL%"
set "PROCABLE_LAUNCHER_PROBE="
if not "%probe_err%"=="0" goto probe_done
findstr /b /c:"procable-engine" "%probe_tmp%" >nul 2>nul
if errorlevel 1 goto probe_done
set "probe_ok=1"
for /f "usebackq tokens=2" %%v in ("%probe_tmp%") do if not defined probe_version set "probe_version=%%v"
:probe_done
del "%probe_tmp%" >nul 2>nul
exit /b 0


:fail
echo procable: no engine binary found (looked in %bin%, %cached%, PATH). 1>&2
echo procable runs offline and does not download its engine. Reinstall with the prompt-orchestrator installer, or set PROCABLE_BIN to a procable engine binary. 1>&2
exit /b 127
