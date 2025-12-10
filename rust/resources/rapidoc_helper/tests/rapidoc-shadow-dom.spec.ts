import { test, expect } from '@playwright/test';

const BASE_URL = 'http://localhost:8080';
const SPEC_URL = 'http://localhost:8080/openapi.json';

test.describe('Rapidoc Shadow DOM Population Tests', () => {
  test('should populate rapi-doc shadow DOM with content from helper.min.js', async ({ page }) => {
    // Navigate to the page
    await page.goto(BASE_URL, { waitUntil: 'domcontentloaded' });
    
    // Capture console messages to verify initialization
    const consoleLogs: string[] = [];
    page.on('console', msg => {
      consoleLogs.push(msg.text());
    });
    
    // Wait for async initialization
    await page.waitForTimeout(3000);
    
    // Get the rapi-doc element
    const rapidocEl = page.locator('rapi-doc#rapidoc');
    
    // Verify the element exists in the DOM
    const elementExists = await rapidocEl.count().then(c => c > 0);
    console.log('✓ rapi-doc element exists in DOM:', elementExists);
    expect(elementExists).toBe(true);
    
    // Check if shadow DOM has been populated with content
    const shadowDOMContent = await page.evaluate(() => {
      const el = document.getElementById('rapidoc') as any;
      if (!el || !el.shadowRoot) {
        return { empty: true, content: '', length: 0 };
      }
      const html = el.shadowRoot.innerHTML;
      return {
        empty: html.trim().length === 0,
        content: html.substring(0, 500),
        length: html.length
      };
    });
    
    console.log('Shadow DOM length:', shadowDOMContent.length, 'bytes');
    console.log('Shadow DOM empty:', shadowDOMContent.empty);
    
    // KEY TEST: The shadow DOM should NOT be empty - this means helper.min.js executed and called loadSpec
    expect(shadowDOMContent.empty).toBe(false);
    expect(shadowDOMContent.length).toBeGreaterThan(0);
  });

  test('should call loadSpec from helper.min.js', async ({ page }) => {
    const consoleLogs: string[] = [];
    page.on('console', msg => {
      consoleLogs.push(msg.text());
    });
    
    await page.goto(BASE_URL, { waitUntil: 'domcontentloaded' });
    await page.waitForTimeout(3000);
    
    // Check that loadSpec was called (via console logs from index.ts)
    const loadSpecCalled = consoleLogs.some(log => log.includes('Calling loadSpec'));
    console.log('✓ loadSpec was called:', loadSpecCalled);
    expect(loadSpecCalled).toBe(true);
    
    // Check for success message
    const specLoaded = consoleLogs.some(log => log.includes('Spec loaded successfully'));
    console.log('✓ Spec loaded successfully:', specLoaded);
    expect(specLoaded).toBe(true);
  });

  test('should fetch OpenAPI spec correctly', async ({ page }) => {
    const consoleLogs: string[] = [];
    page.on('console', msg => {
      consoleLogs.push(msg.text());
    });
    
    await page.goto(BASE_URL, { waitUntil: 'domcontentloaded' });
    await page.waitForTimeout(3000);
    
    // Check console logs for spec fetch confirmation
    const specFetched = consoleLogs.some(log => 
      log.includes('Fetching spec from') || 
      log.includes('OpenAPI spec loaded')
    );
    
    console.log('✓ Spec was fetched:', specFetched);
    expect(specFetched).toBe(true);
    
    // Verify spec contains 32 paths (from our OpenAPI spec)
    const pathsLoaded = consoleLogs.some(log => 
      log.includes('paths:') && log.includes('32')
    );
    
    console.log('✓ 32 API paths loaded:', pathsLoaded);
    expect(pathsLoaded).toBe(true);
  });

  test('should handle initialization errors gracefully', async ({ page }) => {
    const consoleErrors: string[] = [];
    page.on('console', msg => {
      if (msg.type() === 'error') {
        consoleErrors.push(msg.text());
      }
    });
    
    await page.goto(BASE_URL, { waitUntil: 'domcontentloaded' });
    await page.waitForTimeout(2000);
    
    // Filter for critical [rapidoc] errors (not code sample processing warnings)
    const rapidocErrors = consoleErrors.filter(e => 
      e.includes('[rapidoc] Error:')
    );
    
    console.log('Critical rapidoc errors:', rapidocErrors.length);
    if (rapidocErrors.length > 0) {
      rapidocErrors.forEach(err => console.log('ERROR:', err));
    }
    
    // There should be NO critical initialization errors
    expect(rapidocErrors.length).toBe(0);
  });

  test('verify OpenAPI spec is accessible and valid', async ({ request }) => {
    const response = await request.get(SPEC_URL);
    
    console.log('OpenAPI spec response status:', response.status());
    expect(response.status()).toBe(200);
    
    const spec = await response.json();
    
    // Validate OpenAPI structure
    expect(spec).toHaveProperty('openapi');
    expect(spec).toHaveProperty('info');
    expect(spec).toHaveProperty('paths');
    expect(spec.info).toHaveProperty('title', 'rust_photoacoustic');
    
    const pathCount = Object.keys(spec.paths).length;
    console.log('OpenAPI spec paths:', pathCount);
    expect(pathCount).toBe(32);
  });

  test('SPEC_URL should be resolved to /openapi.json', async ({ page }) => {
    await page.goto(BASE_URL, { waitUntil: 'domcontentloaded' });
    
    // Get SPEC_URL value from window
    const specUrl = await page.evaluate(() => (window as any).SPEC_URL);
    console.log('SPEC_URL resolved to:', specUrl);
    
    // Should be /openapi.json (the fallback when {{SPEC_URL}} is not replaced)
    expect(specUrl).toBe('/openapi.json');
  });
});
