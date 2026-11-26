const { describe, it, before, after } = require('node:test');
const assert = require('node:assert');
const http = require('node:http');
const os = require('node:os');
const app = require('./server');

describe('ERP/CRM Application', () => {
  let server;
  let baseUrl;

  before(async () => {
    server = http.createServer(app);
    await new Promise((resolve, reject) => {
      server.listen(0, '127.0.0.1', (err) => {
        if (err) return reject(err);
        const { port } = server.address();
        baseUrl = `http://127.0.0.1:${port}`;
        resolve();
      });
    });
  });

  after(async () => {
    await new Promise((resolve) => server.close(resolve));
  });

  it('should return 200 on /health', async () => {
    const res = await fetch(`${baseUrl}/health`);
    assert.strictEqual(res.status, 200);
    const body = await res.text();
    assert.strictEqual(body, 'OK');
  });

  it('should return instance info on /api/info', async () => {
    const res = await fetch(`${baseUrl}/api/info`);
    assert.strictEqual(res.status, 200);
    const body = await res.json();
    assert.strictEqual(body.version, '1.0.0');
    assert.ok(body.hostname);
    assert.ok(body.platform);
    assert.notStrictEqual(body.hostname, os.hostname());
    assert.match(body.hostname, /^[0-9a-f-]{36}$/);
    assert.strictEqual(body.platform, 'application');
    assert.deepStrictEqual(body.loadavg, []);
    assert.strictEqual(res.headers.get('cache-control'), 'no-store');
  });

  it('should serve the frontend on root', async () => {
    const res = await fetch(`${baseUrl}/`);
    assert.strictEqual(res.status, 200);
    const body = await res.text();
    assert.ok(body.includes('<!DOCTYPE html>') || body.includes('<html'));
  });

  it('protects HTML responses with a CSP and MIME sniffing prevention', async () => {
    const res = await fetch(`${baseUrl}/`);
    assert.strictEqual(res.headers.get('x-powered-by'), null);
    assert.strictEqual(res.headers.get('x-content-type-options'), 'nosniff');
    const csp = res.headers.get('content-security-policy');
    assert.ok(csp.includes("script-src 'self'"));
    assert.ok(csp.includes("frame-ancestors 'none'"));
    if (process.env.NODE_ENV !== 'production') {
      assert.ok(!csp.includes('upgrade-insecure-requests'));
    }
  });

  it('does not serve secrets, source files or unknown API routes', async () => {
    for (const route of ['/.env', '/server.js', '/package.json', '/api/unknown']) {
      const res = await fetch(`${baseUrl}${route}`);
      assert.strictEqual(res.status, 404);
      assert.deepStrictEqual(await res.json(), { error: 'Not found' });
    }
  });

  it('rejects malformed URL encoding without disclosing a stack trace', async () => {
    const res = await fetch(`${baseUrl}/%E0%A4%A`);
    assert.strictEqual(res.status, 404);
    assert.deepStrictEqual(await res.json(), { error: 'Not found' });
  });
});
