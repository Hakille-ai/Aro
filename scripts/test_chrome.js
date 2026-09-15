const { chromium } = require('playwright');

async function test() {
  console.log('Testing in Google Chrome...');
  const browser = await chromium.launch({ channel: 'chrome', headless: true });
  const page = await browser.newPage();

  page.on('request', req => {
    console.log('[REQ]:', req.url());
  });

  page.on('console', msg => {
    console.log(`[CHROME CONSOLE ${msg.type()}]:`, msg.text());
  });

  page.on('pageerror', err => {
    console.error('[CHROME PAGEERROR]:', err.stack || err.message);
  });

  await page.goto('http://127.0.0.1:1420/', { waitUntil: 'load' });
  await page.waitForTimeout(3000);
  await browser.close();
}

test().catch(err => {
  console.error('Test error:', err);
});
