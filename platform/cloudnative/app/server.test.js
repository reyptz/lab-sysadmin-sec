const { describe, it, before, after } = require('node:test');
const assert = require('node:assert');
const http = require('node:http');
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
  });

  it('should serve the frontend on root', async () => {
    const res = await fetch(`${baseUrl}/`);
    assert.strictEqual(res.status, 200);
    const body = await res.text();
    assert.ok(body.includes('<!DOCTYPE html>') || body.includes('<html'));
  });
});
