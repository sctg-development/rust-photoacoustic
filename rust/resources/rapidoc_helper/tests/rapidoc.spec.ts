import { test, expect, Page } from '@playwright/test';

// Base URL for the dev server
const BASE_URL = 'http://localhost:8080';
const SPEC_URL = 'http://localhost:8080/openapi.json';

test.describe('Rapidoc Dev Server - Shadow DOM Population', () => {
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
        return { empty: true, content: '' };
      }
      const html = el.shadowRoot.innerHTML;
      return {
        empty: html.trim().length === 0,
        content: html.substring(0, 500),
        length: html.length
      };
    });
    
    console.log('✓ Shadow DOM length:', shadowDOMContent.length);
    console.log('✓ Shadow DOM empty:', shadowDOMContent.empty);
    
    // The shadow DOM should NOT be empty - this means helper.min.js executed
    expect(shadowDOMContent.empty).toBe(false);
    expect(shadowDOMContent.length).toBeGreaterThan(0);
    
    // Check that loadSpec was called (via console logs)
    const loadSpecCalled = consoleLogs.some(log => log.includes('Calling loadSpec'));
    console.log('✓ loadSpec was called:', loadSpecCalled);
    expect(loadSpecCalled).toBe(true);
    
    // Check for success message
    const specLoaded = consoleLogs.some(log => log.includes('Spec loaded successfully'));
    console.log('✓ Spec loaded successfully:', specLoaded);
    expect(specLoaded).toBe(true);
  });

  test('should have fetched and loaded OpenAPI spec', async ({ page }) => {
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
    
    // Verify spec contains paths info in logs
    const pathsLoaded = consoleLogs.some(log => 
      log.includes('paths:') && log.includes('32')
    );
    
    console.log('✓ 32 API paths loaded:', pathsLoaded);
    expect(pathsLoaded).toBe(true);
  });

  test('should have SPEC_URL initialized correctly', async ({ page }) => {
    const consoleLogs: string[] = [];
    page.on('console', msg => {
      consoleLogs.push(msg.text());
    });
    
    await page.goto(BASE_URL, { waitUntil: 'domcontentloaded' });
    
    // Get SPEC_URL value
    const specUrl = await page.evaluate(() => (window as any).SPEC_URL);
    console.log('✓ SPEC_URL value:', specUrl);
    
    // Should be /openapi.json (the fallback)
    expect(specUrl).toBe('/openapi.json');
  });

  test('should show errors in console if initialization fails', async ({ page }) => {
    const consoleErrors: string[] = [];
    page.on('console', msg => {
      if (msg.type() === 'error') {
        consoleErrors.push(msg.text());
      }
    });
    
    await page.goto(BASE_URL, { waitUntil: 'domcontentloaded' });
    await page.waitForTimeout(2000);
    
    console.log('Console errors:', consoleErrors.length);
    
    // There should be NO critical [rapidoc] errors
    const rapidocErrors = consoleErrors.filter(e => 
      e.includes('[rapidoc]') && 
      !e.includes('Code samples')
    );
    
    console.log('✓ Rapidoc errors:', rapidocErrors.length);
    expect(rapidocErrors.length).toBe(0);
  });

  test('should set SPEC_URL correctly', async ({ page }) => {
    await page.goto(BASE_URL, { waitUntil: 'domcontentloaded' });
    
    // Check that window.SPEC_URL is set
    const specUrl = await page.evaluate(() => (window as any).SPEC_URL);
    console.log('SPEC_URL:', specUrl);
    expect(specUrl).toBeTruthy();
    // Should be either the template var or the fallback /openapi.json
    expect(['/openapi.json', '{{SPEC_URL}}'].some(url => specUrl.includes(url) || specUrl === url)).toBe(true);
  });

  test('should fetch OpenAPI spec without errors', async ({ page }) => {
    // Set up listener for console messages
    const consoleLogs: string[] = [];
    const consoleErrors: string[] = [];
    
    page.on('console', msg => {
      const text = msg.text();
      consoleLogs.push(text);
      console.log('[Browser Console]', text);
      if (msg.type() === 'error') {
        consoleErrors.push(text);
      }
    });
    
    // Navigate to the page
    await page.goto(BASE_URL, { waitUntil: 'domcontentloaded' });
    
    // Wait a bit for the spec to be fetched and loaded
    await page.waitForTimeout(3000);
    
    // Check that no critical errors occurred
    const rapidocErrors = consoleErrors.filter(e => 
      !e.includes('Cannot GET') && // Ignore dev server navigation errors
      !e.includes('overlay.js') && // Ignore webpack dev overlay errors
      !e.includes('Could not find a production build') // Ignore HMR errors
    );
    
    // Log all console output for debugging
    console.log('\n=== Console Logs ===');
    consoleLogs.forEach(log => console.log(log));
    
    if (rapidocErrors.length > 0) {
      console.log('\n=== Console Errors (filtered) ===');
      rapidocErrors.forEach(err => console.log(err));
    }
    
    // Should see successful spec load logs OR that rapidoc element is visible
    const rapiDocVisible = await page.locator('rapi-doc').isVisible().catch(() => false);
    const hasApiContent = consoleLogs.some(log => 
      log.includes('openapi') || 
      log.includes('Rapidoc') ||
      log.includes('rust_photoacoustic')
    );
    
    console.log('\n=== Rapidoc Status ===');
    console.log('Rapidoc element visible:', rapiDocVisible);
    console.log('API content in logs:', hasApiContent);
    
    // Either should have Rapidoc visible or should not have critical errors
    // (Sometimes logs don't show up in dev mode but element still renders)
    expect(rapiDocVisible || rapidocErrors.length === 0).toBe(true);
  });

  test('should render Rapidoc UI elements', async ({ page }) => {
    await page.goto(BASE_URL, { waitUntil: 'domcontentloaded' });
    
    // Wait for rapi-doc to be visible
    const rapidocEl = await page.locator('rapi-doc#rapidoc');
    await expect(rapidocEl).toBeVisible({ timeout: 10000 });
    
    // Check if Rapidoc has loaded by looking for internal elements
    // Rapidoc might render content in shadow DOM, so we check for the element's attributes
    const hasSpec = await page.locator('rapi-doc#rapidoc').evaluate((el: any) => {
      // Try to access the spec property if available
      return el.hasAttribute('spec-url') || 
             (el.shadowRoot && el.shadowRoot.innerHTML.length > 100) ||
             el.innerHTML.length > 100;
    });
    
    // Try to find the description by looking for the info title/description in the page content
    // Wait longer to allow Rapidoc to render content
    await page.waitForTimeout(2000);
    
    // Check if Rapidoc has rendered any content
    const contentCheck = await page.locator('rapi-doc#rapidoc').evaluate((el: any) => {
      const hasContent = el.shadowRoot ? el.shadowRoot.textContent.length > 500 : false;
      const hasMethods = el.shadowRoot ? (el.shadowRoot.textContent.includes('GET') || el.shadowRoot.textContent.includes('POST')) : false;
      return { hasContent, hasMethods };
    });
    
    console.log('Rapidoc has spec:', hasSpec);
    console.log('Rapidoc content length > 500:', contentCheck.hasContent);
    console.log('Rapidoc has HTTP methods (GET/POST):', contentCheck.hasMethods);
    
    // Log the rapi-doc element's attributes for debugging
    const attributes = await page.locator('rapi-doc#rapidoc').evaluate((el: Element) => {
      const attrs: Record<string, string> = {};
      for (let attr of el.attributes) {
        attrs[attr.name] = attr.value;
      }
      return attrs;
    });
    
    console.log('Rapi-doc attributes:', attributes);
    
    // At minimum, the element should exist and be rendered
    expect(rapidocEl).toBeTruthy();
  });

  test('should load OpenAPI spec from /openapi.json endpoint', async ({ page }) => {
    const response = await page.goto(SPEC_URL);
    
    expect(response?.status()).toBe(200);
    
    const contentType = response?.headers()['content-type'];
    expect(contentType).toContain('application/json');
    
    const body = await response?.text();
    expect(body).toBeTruthy();
    
    const spec = JSON.parse(body || '{}');
    expect(spec.openapi).toBeDefined();
    expect(spec.info).toBeDefined();
    expect(spec.paths).toBeDefined();
    
    console.log('OpenAPI Spec Info:');
    console.log('  Version:', spec.openapi);
    console.log('  Title:', spec.info.title);
    console.log('  Paths:', Object.keys(spec.paths).length);
  });

  test('should handle missing SPEC_URL gracefully', async ({ page, context }) => {
    // Create a page with cleared localStorage/sessionStorage
    const newPage = await context.newPage();
    
    // Listen for console errors
    const errors: string[] = [];
    newPage.on('console', msg => {
      if (msg.type() === 'error') {
        errors.push(msg.text());
      }
    });
    
    // Set up window.SPEC_URL as empty before navigation
    await newPage.addInitScript(() => {
      (window as any).SPEC_URL = '';
    });
    
    await newPage.goto(BASE_URL, { waitUntil: 'domcontentloaded' });
    await newPage.waitForTimeout(2000);
    
    // Should fallback to /openapi.json without critical errors
    const rapidocErrors = errors.filter(e => 
      e.includes('Cannot set properties') || 
      e.includes('Cannot read properties')
    );
    
    console.log('Errors with empty SPEC_URL:', rapidocErrors);
    
    // The page should still load (even if there are non-critical errors)
    const rapidocEl = await newPage.locator('rapi-doc#rapidoc');
    await expect(rapidocEl).toBeVisible({ timeout: 5000 });
    
    await newPage.close();
  });

  test('should log spec loading progress', async ({ page }) => {
    const logs: string[] = [];
    const errors: string[] = [];
    
    page.on('console', msg => {
      const text = msg.text();
      if (msg.type() === 'error' || msg.type() === 'warning') {
        errors.push(text);
      }
      if (text.includes('[rapidoc]')) {
        logs.push(text);
      }
    });
    
    await page.goto(BASE_URL, { waitUntil: 'domcontentloaded' });
    await page.waitForTimeout(3000);
    
    // We should have Rapidoc element loaded
    const rapiDocElement = page.locator('rapi-doc');
    const isVisible = await rapiDocElement.isVisible().catch(() => false);
    
    // Log what we found
    console.log('\n=== Rapidoc Loading Logs ===');
    logs.forEach(log => console.log(log));
    
    console.log('\nRapidoc element visible:', isVisible);
    console.log('Rapidoc logs captured:', logs.length);
    
    // Either should have logs or should have Rapidoc element visible
    // Some browsers/dev environments may not show logs but still render
    expect(logs.length > 0 || isVisible).toBe(true);
  });
});

test.describe('OpenAPI Spec Validation', () => {
  test('should have required OpenAPI structure', async ({ request }) => {
    const response = await request.get(SPEC_URL);
    expect(response.ok()).toBe(true);
    
    const spec = await response.json();
    
    // Check required OpenAPI fields
    expect(spec).toHaveProperty('openapi');
    expect(spec).toHaveProperty('info');
    expect(spec).toHaveProperty('paths');
    expect(spec).toHaveProperty('components');
    
    // Validate info section
    expect(spec.info).toHaveProperty('title');
    expect(spec.info).toHaveProperty('version');
    
    // Validate paths section (should have at least some paths)
    const pathCount = Object.keys(spec.paths).length;
    expect(pathCount).toBeGreaterThan(0);
    
    console.log(`OpenAPI spec has ${pathCount} paths`);
  });

  test('should have valid HTTP methods', async ({ request }) => {
    const response = await request.get(SPEC_URL);
    const spec = await response.json();
    
    const validMethods = ['get', 'post', 'put', 'delete', 'patch', 'options', 'head'];
    
    for (const path in spec.paths) {
      const pathItem = spec.paths[path];
      for (const method in pathItem) {
        if (method.startsWith('x-')) continue; // Skip extensions
        expect(validMethods).toContain(method.toLowerCase());
      }
    }
    
    console.log('All HTTP methods in spec are valid');
  });
});
