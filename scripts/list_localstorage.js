const fs = require('fs');

const content = fs.readFileSync('apps/desktop/src/App.svelte', 'utf8');
const lines = content.split('\n');

lines.forEach((line, idx) => {
  if (line.includes('localStorage.getItem')) {
    console.log(`Line ${idx + 1}: ${line.trim()}`);
  }
});
