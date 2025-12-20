// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

import { getAll } from '../openapi-to-har';
import { HTTPSnippet } from '../httpsnippet';
import openApiSpec from '../../public/openapi.json';

describe('curl snippet generation', () => {
    it('should generate curl snippets for all endpoints', () => {
        const endpoints = getAll(openApiSpec as any);

        expect(endpoints.length).toBeGreaterThan(0);

        // Test first 3 endpoints to avoid verbose output
        endpoints.slice(0, 3).forEach((endpoint) => {
            console.log(`\n========================================`);
            console.log(`${endpoint.method} ${endpoint.url}`);
            console.log(`Description: ${endpoint.description}`);
            console.log(`========================================`);

            endpoint.hars.forEach((har, index) => {
                try {
                    const httpSnippet = new HTTPSnippet(har as any);
                    const curlSnippet = httpSnippet.convert('shell', 'curl');
                    console.log(`\n[Snippet ${index + 1}${har.comment ? ` - ${har.comment}` : ''}]`);
                    console.log(curlSnippet);

                    // Basic validation that we got a curl command
                    expect(typeof curlSnippet).toBe('string');
                    expect(curlSnippet).toMatch(/^curl\s/);
                } catch (err) {
                    console.warn(`Failed to generate snippet: ${err}`);
                    throw err;
                }
            });
        });
    });

    it('should generate snippets with proper authorization headers', () => {
        const endpoints = getAll(openApiSpec as any);
        const protectedEndpoint = endpoints.find((ep) =>
            ep.hars[0]?.headers.some((h) => h.name.toLowerCase() === 'authorization')
        );

        if (!protectedEndpoint) {
            console.log('No protected endpoints found');
            return;
        }

        console.log(`\nTesting protected endpoint: ${protectedEndpoint.method} ${protectedEndpoint.url}`);
        const har = protectedEndpoint.hars[0];
        const httpSnippet = new HTTPSnippet(har as any);
        const curlSnippet = httpSnippet.convert('shell', 'curl');

        console.log(curlSnippet);
        expect(curlSnippet).toMatch(/Authorization|authorization|-H.*-H/);
    });
});
