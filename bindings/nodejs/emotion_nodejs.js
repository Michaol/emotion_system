/* eslint-disable */
// Auto-generated loader for NAPI-RS native addon
const { join } = require('path');
const { existsSync } = require('fs');

function loadNativeAddon() {
  const candidates = [
    join(__dirname, 'emotion_nodejs.win32-x64-msvc.node'),
    join(__dirname, 'emotion_nodejs.node'),
  ];

  for (const candidate of candidates) {
    if (existsSync(candidate)) {
      return require(candidate);
    }
  }

  throw new Error(
    'Failed to load native addon. Run `cargo build -p emotion-nodejs` first.'
  );
}

module.exports = loadNativeAddon();
