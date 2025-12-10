import { test, expect } from '@playwright/test';

const BASE_URL = 'http://localhost:8080';

test('Debug webpack serve response', async ({ page }) => {
  // Capture all console messages and requests
  const consoleLogs: string[] = [];
  const requests: string[] = [];
  const errors: string[] = [];
  
  page.on('console', msg => {
    consoleLogs.push(`[${msg.type()}] ${msg.text()}`);
  });
  
  page.on('response', response => {
    requests.push(`${response.status()} ${response.url()}`);
  });

  // Capture uncaught exceptions
  page.on('pageerror', err => {
    errors.push(`ERROR: ${err.message}`);
  });
  
  console.log('\n=== NAVIGATING TO ', BASE_URL, ' ===');
  const response = await page.goto(BASE_URL);
  console.log('Response status:', response?.status());
  console.log('Response URL:', response?.url());
  
  // Get HTML content
  const html = await page.content();
  console.log('\nHTML content length:', html.length);
  console.log('HTML starts with:', html.substring(0, 200));
  console.log('HTML contains "rapi-doc":', html.includes('rapi-doc'));
  console.log('HTML contains "helper.min.js":', html.includes('helper.min.js'));
  console.log('HTML contains "rapidoc":', html.includes('rapidoc'));
  
  // Wait a bit for scripts to load
  await page.waitForTimeout(2000);
  
  // Print console logs
  console.log('\n=== CONSOLE LOGS ===');
  consoleLogs.forEach(log => console.log(log));
  
  // Print requests
  console.log('\n=== NETWORK REQUESTS ===');
  requests.forEach(req => console.log(req));
  
  // Check for script tags
  const scriptSrc = await page.evaluate(() => {
    const scripts = Array.from(document.querySelectorAll('script'));
    return scripts.map(s => ({
      src: s.src,
      textContent: s.textContent?.substring(0, 100) || 'N/A'
    }));
  });
  
  console.log('\n=== SCRIPT TAGS ===');
  scriptSrc.forEach((s, i) => {
    console.log(`Script ${i}: src="${s.src}" text="${s.textContent}"`);
  });
  
  // Check if helper.min.js was loaded
  const helperLoaded = await page.evaluate(() => {
    return (window as any).__webpack_require__ !== undefined;
  });
  
  console.log('\n=== WEBPACK STATUS ===');
  console.log('Webpack modules loaded:', helperLoaded);
  
  // Try to access window object
  const windowProps = await page.evaluate(() => {
    return {
      hasRapidoc: (window as any).rapi_doc !== undefined,
      hasInitialize: (window as any).initializeRapidoc !== undefined,
      hasSpecUrl: (window as any).SPEC_URL !== undefined,
      SPEC_URL: (window as any).SPEC_URL,
      hasElement: document.getElementById('rapidoc') !== null,
      elementTagName: document.getElementById('rapidoc')?.tagName || 'NOT_FOUND',
      documentReady: document.readyState,
    };
  });
  
  console.log('\n=== WINDOW PROPERTIES ===');
  console.log('Window properties:', windowProps);

  if (errors.length > 0) {
    console.log('\n=== ERRORS ===');
    errors.forEach(err => console.log(err));
  }
  
  expect(html.includes('rapi-doc')).toBe(true);
});
