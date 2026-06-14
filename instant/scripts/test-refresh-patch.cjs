const fs = require('fs');
const path = require('path');

const runtimePath = path.join(__dirname, '..', 'node_modules', '@vitejs', 'plugin-react', 'dist', 'refresh-runtime.js');

function safeReplace(content) {
  return content.replace(/Object\.getOwnPropertyNames\(type\.prototype\)/g, "Object.getOwnPropertyNames((type && type.prototype) ? type.prototype : {})");
}

function runOnSample() {
  const sample = 'const props = Object.getOwnPropertyNames(type.prototype);';
  const patched = safeReplace(sample);
  if (!sample.includes('type.prototype')) {
    console.error('sample does not contain expected pattern');
    process.exit(2);
  }
  if (patched.includes('type.prototype')) {
    console.error('patch failed on sample');
    process.exit(3);
  }
  console.log('sample patch OK');
}

try {
  if (!fs.existsSync(runtimePath)) {
    console.log('[test-refresh-patch] runtime file not found, running sample-only test');
    runOnSample();
    process.exit(0);
  }

  const original = fs.readFileSync(runtimePath, 'utf8');
  if (!original.includes('Object.getOwnPropertyNames(type.prototype)')) {
    console.log('[test-refresh-patch] no unsafe pattern found in runtime; nothing to test');
    process.exit(0);
  }

  const patched = safeReplace(original);
  if (patched.includes('Object.getOwnPropertyNames(type.prototype)')) {
    console.error('[test-refresh-patch] pattern still present after patch');
    process.exit(1);
  }

  console.log('[test-refresh-patch] runtime patch verification passed');
  process.exit(0);
} catch (err) {
  console.error('[test-refresh-patch] error:', err);
  process.exit(4);
}
