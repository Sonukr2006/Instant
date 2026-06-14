const fs = require('fs');
const path = require('path');

// Locate plugin-react refresh-runtime file inside node_modules
const runtimePath = path.join(__dirname, '..', 'node_modules', '@vitejs', 'plugin-react', 'dist', 'refresh-runtime.js');

function safeReplace(content) {
  // Guard direct prototype access
  content = content.replace(/Object\.getOwnPropertyNames\(type\.prototype\)/g, "Object.getOwnPropertyNames((type && type.prototype) ? type.prototype : {})");
  // Wrap risky prototype access in try/catch where pattern appears
  content = content.replace(/(var\s+propsNames\s*=\s*)Object\.getOwnPropertyNames\(\(type\s*&&\s*type\.prototype\)\s*\?\s*type\.prototype\s*:\s*\{\}\);/g, "$1(() => { try { return Object.getOwnPropertyNames((type && type.prototype) ? type.prototype : {}); } catch (e) { return []; } })();");
  return content;
}

try {
  if (!fs.existsSync(runtimePath)) {
    console.log('[patch-refresh-runtime] runtime file not found:', runtimePath);
    process.exit(0);
  }

  const original = fs.readFileSync(runtimePath, 'utf8');
  const patched = safeReplace(original);

  if (original === patched) {
    console.log('[patch-refresh-runtime] no changes required');
    process.exit(0);
  }

  fs.writeFileSync(runtimePath, patched, 'utf8');
  console.log('[patch-refresh-runtime] patched', runtimePath);
} catch (err) {
  console.error('[patch-refresh-runtime] failed:', err);
  process.exit(1);
}
