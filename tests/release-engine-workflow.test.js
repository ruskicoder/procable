import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';

const workflow = Bun.YAML.parse(readFileSync(new URL('../.github/workflows/release-engine.yml', import.meta.url), 'utf8'));
const action = (job, name) => job.steps.find(step => step.uses?.startsWith(`${name}@`));

describe('engine release boundary (procable: unsigned, no OIDC)', () => {
  test('no job receives an OIDC token and only publish can write', () => {
    expect(workflow.on).toEqual({ push: { tags: ['engine-v*'] } });
    expect(workflow.permissions).toEqual({ contents: 'read' });
    expect(workflow.jobs['sign-windows']).toBeUndefined();
    for (const job of Object.values(workflow.jobs)) {
      expect(job.permissions?.['id-token']).toBeUndefined();
    }
    expect(workflow.jobs.publish.permissions).toEqual({ contents: 'write' });
  });

  test('publication waits for every build and collects only engine artifacts', () => {
    expect(action(workflow.jobs.build, 'actions/upload-artifact').with.name).toBe('procable-${{ matrix.short }}');
    expect(workflow.jobs.publish.needs).toEqual(['build']);
    expect(action(workflow.jobs.publish, 'actions/download-artifact').with.pattern).toBe('procable-*');
    expect(workflow.jobs.publish.steps.find(step => step.name === 'Lay out release assets with checksums').run).toContain('sha256sum');
  });

  test('artifact downloads stay on the same-run runtime-token path', () => {
    const download = action(workflow.jobs.publish, 'actions/download-artifact');
    // Supplying github-token opts into the public API path, which requires
    // separate Actions permissions and can read other workflow runs.
    for (const input of ['github-token', 'repository', 'run-id']) {
      expect(download.with[input]).toBeUndefined();
    }
  });

  test('the release is created in one gh call that publishes only after every asset is up', () => {
    const run = workflow.jobs.publish.steps.find(step => step.name === 'Publish the GitHub Release').run;
    // gh only drafts, uploads, then publishes when the assets are passed to create.
    expect(run).toMatch(/gh release create[\s\S]*\bout\/\*/);
    for (const flag of ['gh release upload', '--draft']) expect(run).not.toContain(flag);
  });

  test('every third-party action is pinned to a commit', () => {
    for (const job of Object.values(workflow.jobs)) {
      for (const step of job.steps) {
        if (step.uses) expect(step.uses).toMatch(/@[a-f0-9]{40}$/);
      }
    }
  });
});
