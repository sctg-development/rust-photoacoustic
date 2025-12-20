import {
    createHarParameterObjects,
    getAll,
    getEndpoint,
} from '../openapi-to-har';
import openApiSpec from '../../public/openapi.json';

describe('openapi-to-har', () => {
    describe('createHarParameterObjects', () => {
        it('should create HAR parameter object for simple string value', () => {
            const param = {
                name: 'id',
                in: 'path' as const,
                type: 'string',
            };

            const result = createHarParameterObjects(param as any, '123');

            expect(result).toHaveLength(1);
            expect(result[0].name).toBe('id');
            expect(typeof result[0].value).toBe('string');
        });

        it('should create HAR parameter object for query parameter', () => {
            const param = {
                name: 'filter',
                in: 'query' as const,
                type: 'string',
            };

            const result = createHarParameterObjects(param, 'active');

            expect(result).toHaveLength(1);
            expect(result[0]).toEqual({
                name: 'filter',
                value: 'active',
            });
        });

        it('should handle array values with explode=true', () => {
            const param = {
                name: 'tags',
                in: 'query' as const,
                style: 'form',
                explode: true,
                type: 'array',
            };

            const result = createHarParameterObjects(param, ['tag1', 'tag2']);

            expect(result).toHaveLength(2);
            expect(result[0]).toEqual({ name: 'tags', value: 'tag1' });
            expect(result[1]).toEqual({ name: 'tags', value: 'tag2' });
        });

        it('should handle array values with explode=false', () => {
            const param = {
                name: 'tags',
                in: 'query' as const,
                style: 'form',
                explode: false,
                type: 'array',
            };

            const result = createHarParameterObjects(param, ['tag1', 'tag2']);

            expect(result).toHaveLength(1);
            expect(result[0].value).toContain('tag1');
            expect(result[0].value).toContain('tag2');
        });

        it('should handle object parameters', () => {
            const param = {
                name: 'filter',
                in: 'query' as const,
                style: 'form',
                explode: true,
                type: 'object',
            };

            const result = createHarParameterObjects(param, { status: 'active', type: 'user' });

            expect(result).toHaveLength(2);
            expect(result.some((r) => r.name === 'status' && r.value === 'active')).toBe(true);
            expect(result.some((r) => r.name === 'type' && r.value === 'user')).toBe(true);
        });

        it('should throw error if required parameters are missing', () => {
            const param = {
                in: 'query' as const,
            } as any;

            expect(() => {
                createHarParameterObjects(param, 'value');
            }).toThrow();
        });

        it('should handle header parameters', () => {
            const param = {
                name: 'X-Custom-Header',
                in: 'header' as const,
                type: 'string',
            };

            const result = createHarParameterObjects(param, 'custom-value');

            expect(result).toHaveLength(1);
            expect(result[0]).toEqual({
                name: 'X-Custom-Header',
                value: 'custom-value',
            });
        });

        it('should handle cookie parameters', () => {
            const param = {
                name: 'session_id',
                in: 'cookie' as const,
                type: 'string',
            };

            const result = createHarParameterObjects(param, 'abc123');

            expect(result).toHaveLength(1);
            expect(result[0]).toEqual({
                name: 'session_id',
                value: 'abc123',
            });
        });
    });

    describe('getEndpoint', () => {
        it('should create HAR request for GET endpoint', () => {
            const result = getEndpoint(openApiSpec as any, '/api/config', 'get');

            expect(Array.isArray(result)).toBe(true);
            expect(result.length).toBeGreaterThan(0);

            const har = result[0];
            expect(har.method).toBe('GET');
            expect(har.url).toContain('/api/config');
            expect(har.httpVersion).toBe('HTTP/1.1');
            expect(har.headers).toBeDefined();
            expect(Array.isArray(har.headers)).toBe(true);
        });

        it('should create HAR request with proper authentication headers', () => {
            const result = getEndpoint(openApiSpec as any, '/api/config', 'get');
            const har = result[0];

            // Should have Authorization header for Bearer token
            const authHeader = har.headers.find((h) => h.name === 'Authorization');
            expect(authHeader).toBeDefined();
            expect(authHeader?.value).toContain('Bearer');
        });

        it('should create HAR request with query parameters', () => {
            const result = getEndpoint(openApiSpec as any, '/api/action/{node_id}/history', 'get', {
                node_id: 'redis_stream_action',
                limit: '10',
            });

            expect(result).toBeDefined();
            expect(Array.isArray(result)).toBe(true);
        });

        it('should create HAR request with POST body for POST endpoints', () => {
            const result = getEndpoint(openApiSpec as any, '/api/graph/config', 'post');

            expect(result.length).toBeGreaterThan(0);
            const har = result[0];
            expect(har.method).toBe('POST');
            expect(har.postData).toBeDefined();
            expect(har.postData?.mimeType).toBe('application/json');
            expect(har.postData?.text).toBeDefined();
        });

        it('should set proper content-type header for POST requests', () => {
            const result = getEndpoint(openApiSpec as any, '/api/graph/config', 'post');
            const har = result[0];

            const contentTypeHeader = har.headers.find(
                (h) => h.name.toLowerCase() === 'content-type'
            );
            expect(contentTypeHeader).toBeDefined();
            expect(contentTypeHeader?.value).toBe('application/json');
        });

        it('should include request body size information', () => {
            const result = getEndpoint(openApiSpec as any, '/api/config', 'get');
            const har = result[0];

            expect(har.bodySize).toBeDefined();
            expect(typeof har.bodySize).toBe('number');
        });
    });

    describe('getAll', () => {
        it('should return array of all endpoints from OpenAPI spec', () => {
            const result = getAll(openApiSpec as any);

            expect(Array.isArray(result)).toBe(true);
            expect(result.length).toBeGreaterThan(0);
        });

        it('should include all paths from the OpenAPI spec', () => {
            const result = getAll(openApiSpec as any);

            expect(result.some((ep) => ep.url.includes('/api/config'))).toBe(true);
            expect(result.some((ep) => ep.url.includes('/api/graph'))).toBe(true);
            expect(result.some((ep) => ep.url.includes('/api/system/stats'))).toBe(true);
        });

        it('should create HAR objects for each endpoint', () => {
            const result = getAll(openApiSpec as any);

            result.forEach((endpoint) => {
                expect(endpoint.method).toBeDefined();
                expect(['GET', 'POST', 'PUT', 'DELETE', 'PATCH', 'HEAD', 'OPTIONS']).toContain(
                    endpoint.method
                );
                expect(endpoint.url).toBeDefined();
                expect(typeof endpoint.url).toBe('string');
                expect(endpoint.description).toBeDefined();
                expect(Array.isArray(endpoint.hars)).toBe(true);
                expect(endpoint.hars.length).toBeGreaterThan(0);
            });
        });

        it('should include proper descriptions for endpoints', () => {
            const result = getAll(openApiSpec as any);
            const configEndpoint = result.find((ep) => ep.url.includes('/api/config'));

            expect(configEndpoint?.description).toBeDefined();
            expect(configEndpoint?.description).not.toBe('No description available');
        });

        it('should handle endpoints with no description gracefully', () => {
            const result = getAll(openApiSpec as any);

            // All endpoints should have some description
            result.forEach((endpoint) => {
                expect(endpoint.description).toBeDefined();
                expect(typeof endpoint.description).toBe('string');
            });
        });

        it('should create proper HAR objects for all endpoints', () => {
            const result = getAll(openApiSpec as any);

            result.forEach((endpoint) => {
                endpoint.hars.forEach((har) => {
                    expect(har.method).toBeDefined();
                    expect(har.url).toBeDefined();
                    expect(har.headers).toBeDefined();
                    expect(Array.isArray(har.headers)).toBe(true);
                    expect(har.queryString).toBeDefined();
                    expect(Array.isArray(har.queryString)).toBe(true);
                    expect(har.cookies).toBeDefined();
                    expect(Array.isArray(har.cookies)).toBe(true);
                    expect(har.httpVersion).toBe('HTTP/1.1');
                    expect(typeof har.headersSize).toBe('number');
                    expect(typeof har.bodySize).toBe('number');
                });
            });
        });
    });

    describe('OpenAPI spec compatibility', () => {
        it('should parse the real openapi.json spec without errors', () => {
            expect(() => {
                getAll(openApiSpec as any);
            }).not.toThrow();
        });

        it('should handle OpenAPI 3.0.0 format', () => {
            expect(openApiSpec.openapi).toBe('3.0.0');
            const result = getAll(openApiSpec as any);
            expect(result.length).toBeGreaterThan(0);
        });

        it('should extract server URLs from spec', () => {
            const result = getAll(openApiSpec as any);

            // URLs should be properly formed
            result.forEach((endpoint) => {
                expect(endpoint.url).toMatch(/^(https?:\/\/|\/)/);
            });
        });

        it('should handle all HTTP methods in the spec', () => {
            const result = getAll(openApiSpec as any);
            const methods = new Set(result.map((ep) => ep.method));

            expect(methods.size).toBeGreaterThan(0);
            methods.forEach((method) => {
                expect(['GET', 'POST', 'PUT', 'DELETE', 'PATCH', 'HEAD', 'OPTIONS']).toContain(
                    method
                );
            });
        });

        it('should include authentication headers for secured endpoints', () => {
            const result = getAll(openApiSpec as any);

            // Find endpoints that have security requirements (they should have auth headers)
            const securedEndpoints = result.filter((ep) => {
                const pathKey = ep.url.replace(/^https?:\/\/[^/]+/, ''); // Remove base URL
                const pathObj = (openApiSpec as any).paths[pathKey];
                if (!pathObj) return false;

                // Extract method from the HAR request
                const method = ep.method.toLowerCase();
                const methodObj = pathObj[method];

                // Check if this endpoint has security defined
                return methodObj && methodObj.security && methodObj.security.length > 0;
            });

            // Verify all secured endpoints have Authorization headers
            securedEndpoints.forEach((endpoint) => {
                endpoint.hars.forEach((har) => {
                    const hasAuth = har.headers.some(
                        (h) =>
                            h.name.toLowerCase() === 'authorization' ||
                            h.name.toLowerCase().includes('api-key')
                    );

                    expect(hasAuth).toBe(true);
                });
            });

            // Verify we actually found some secured endpoints to test
            expect(securedEndpoints.length).toBeGreaterThan(0);
        });

        it('should handle request bodies for POST endpoints', () => {
            const result = getAll(openApiSpec as any);
            const postEndpoints = result.filter((ep) => ep.method === 'POST');

            postEndpoints.forEach((endpoint) => {
                endpoint.hars.forEach((har) => {
                    if (har.postData) {
                        expect(har.postData.mimeType).toBeDefined();
                        expect(['application/json', 'application/x-www-form-urlencoded', 'multipart/form-data']).toContain(
                            har.postData.mimeType
                        );
                    }
                });
            });
        });
    });

    describe('Edge cases', () => {
        it('should handle endpoints with path parameters', () => {
            const result = getEndpoint(openApiSpec as any, '/api/action/{node_id}/history', 'get');

            expect(result).toBeDefined();
            expect(Array.isArray(result)).toBe(true);
            expect(result.length).toBeGreaterThan(0);
        });

        it('should handle numeric values in parameters', () => {
            const param = {
                name: 'page',
                in: 'query' as const,
                type: 'number',
            };

            const result = createHarParameterObjects(param as any, 42);

            expect(result).toHaveLength(1);
            expect(result[0].value).toBe('42');
        });

        it('should handle boolean values in parameters', () => {
            const param = {
                name: 'active',
                in: 'query' as const,
                type: 'boolean',
            };

            const result = createHarParameterObjects(param as any, true);

            expect(result).toHaveLength(1);
            expect(result[0].value).toBe('true');
        });

        it('should handle null values gracefully', () => {
            const param = {
                name: 'filter',
                in: 'query' as const,
                type: 'string',
            };

            const result = createHarParameterObjects(param as any, null);

            expect(result).toHaveLength(1);
            expect(result[0].value).toBe('null');
        });
    });

    describe('HAR format compliance', () => {
        it('should generate valid HAR 1.2 format requests', () => {
            const result = getEndpoint(openApiSpec as any, '/api/config', 'get');
            const har = result[0];

            // HAR 1.2 required fields
            expect(har.method).toBeDefined();
            expect(['GET', 'POST', 'PUT', 'DELETE', 'PATCH', 'HEAD', 'OPTIONS']).toContain(
                har.method
            );
            expect(har.url).toBeDefined();
            expect(typeof har.url).toBe('string');
            expect(har.httpVersion).toBe('HTTP/1.1');
            expect(Array.isArray(har.headers)).toBe(true);
            expect(Array.isArray(har.queryString)).toBe(true);
            expect(Array.isArray(har.cookies)).toBe(true);
            expect(typeof har.headersSize).toBe('number');
            expect(typeof har.bodySize).toBe('number');
        });

        it('should include proper HAR parameter objects', () => {
            const result = getEndpoint(openApiSpec as any, '/api/config', 'get');
            const har = result[0];

            har.headers.forEach((header) => {
                expect(header.name).toBeDefined();
                expect(typeof header.name).toBe('string');
                expect(header.value).toBeDefined();
                expect(typeof header.value).toBe('string');
            });
        });
    });
});
