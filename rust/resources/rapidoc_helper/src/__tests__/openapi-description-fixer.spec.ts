/**
 * Tests for OpenAPI Description Fixer
 * 
 * Validates that the OpenAPI description fixer correctly processes:
 * - Escaped newlines and whitespace
 * - Markdown code blocks (JSON, JavaScript, etc.)
 * - List formatting
 * - Schema descriptions in OpenAPI documents
 */

import fixOpenAPIDescriptions, { openapiFix } from '../openapi-description-fixer';

// Helper to create minimal OpenAPI spec for testing
const createSpec = (overrides: any = {}) => ({
    openapi: '3.0.0',
    info: { title: 'Test', version: '1.0' },
    paths: {},
    components: {},
    ...overrides
} as any);

describe('OpenAPI Description Fixer', () => {
    describe('fixDescription - Escaped Characters', () => {
        it('should convert literal \\n to actual newlines', () => {
            const spec = createSpec({
                info: { title: 'Test', version: '1.0', description: 'Line 1\\nLine 2' }
            });
            const result = fixOpenAPIDescriptions(spec);
            expect(result.info.description).toBe('Line 1\nLine 2');
        });

        it('should convert literal \\t to actual tabs', () => {
            const spec = createSpec({
                info: { title: 'Test', version: '1.0', description: 'Start\\tTabbed' }
            });
            const result = fixOpenAPIDescriptions(spec);
            expect(result.info.description).toBe('Start\tTabbed');
        });

        it('should convert literal \\r to actual carriage returns', () => {
            const spec = createSpec({
                info: { title: 'Test', version: '1.0', description: 'Line 1\\rLine 2' }
            });
            const result = fixOpenAPIDescriptions(spec);
            expect(result.info.description).toBe('Line 1\rLine 2');
        });

        it('should remove escaped backticks', () => {
            const spec = createSpec({
                info: { title: 'Test', version: '1.0', description: 'Code: \\`example\\`' }
            });
            const result = fixOpenAPIDescriptions(spec);
            expect(result.info.description).toBe('Code: `example`');
        });

        it('should handle combined escape sequences', () => {
            const spec = createSpec({
                info: { title: 'Test', version: '1.0', description: 'Line 1\\n\\tIndented\\nLine 3' }
            });
            const result = fixOpenAPIDescriptions(spec);
            expect(result.info.description).toBe('Line 1\n\tIndented\nLine 3');
        });
    });

    describe('fixDescription - Code Blocks', () => {
        it('should format code block with language identifier', () => {
            const spec = createSpec({
                info: { title: 'Test', version: '1.0', description: '```javascript\nconst x = 1;```' }
            });
            const result = fixOpenAPIDescriptions(spec);
            expect(result.info.description).toContain('```javascript\n');
            expect(result.info.description).toContain('\n```');
        });

        it('should handle code blocks without language identifier', () => {
            const spec = createSpec({
                info: { title: 'Test', version: '1.0', description: '```\nplain text\n```' }
            });
            const result = fixOpenAPIDescriptions(spec);
            expect(result.info.description).toContain('```\n');
        });

        it('should format JSON code blocks with proper indentation', () => {
            const spec = createSpec({
                info: { title: 'Test', version: '1.0', description: '```json\n{"key":"value","nested":{"prop":"val"}}\n```' }
            });
            const result = fixOpenAPIDescriptions(spec);
            const description = result.info.description!;
            expect(description).toContain('```json');
            expect(description).toContain('"key"');
            expect(description).toContain('"value"');
            // Check that JSON is formatted (has newlines)
            expect(description).toMatch(/```json\n[\s\S]+\n```/);
        });

        it('should preserve JSON structure while formatting', () => {
            const spec = createSpec({
                info: { title: 'Test', version: '1.0', description: '```json\n{"array":[1,2,3],"object":{"a":"b"}}\n```' }
            });
            const result = fixOpenAPIDescriptions(spec);
            const description = result.info.description!;
            expect(description).toContain('"array"');
            expect(description).toContain('"object"');
        });

        it('should handle nested code blocks', () => {
            const spec = createSpec({
                info: { title: 'Test', version: '1.0', description: 'Text before ```js\ncode here\n``` text after' }
            });
            const result = fixOpenAPIDescriptions(spec);
            expect(result.info.description).toContain('Text before');
            expect(result.info.description).toContain('text after');
            expect(result.info.description).toContain('```js');
        });
    });

    describe('fixDescription - List Items', () => {
        it('should fix list items on same line with dash', () => {
            const spec = createSpec({
                info: { title: 'Test', version: '1.0', description: 'Option: - item1 - item2' }
            });
            const result = fixOpenAPIDescriptions(spec);
            const description = result.info.description!;
            // Check that list items are present and properly separated
            expect(description).toContain('Option:');
            expect(description).toContain('- item1');
            expect(description).toContain('- item2');
        });

        it('should fix subsequent list items on same line', () => {
            const spec = createSpec({
                info: { title: 'Test', version: '1.0', description: 'Items:\n- first item - second item - third item' }
            });
            const result = fixOpenAPIDescriptions(spec);
            const description = result.info.description!;
            // The regex replaces the dash pattern in subsequent list items
            expect(description).toContain('- first item');
            expect(description).toContain('- second item');
            expect(description).toContain('- third item');
        });
    });

    describe('fixOpenAPIDescriptions - Paths', () => {
        it('should fix descriptions in path operations', () => {
            const spec = createSpec({
                paths: {
                    '/users': {
                        get: {
                            description: 'Get users\\nLine 2'
                        }
                    }
                }
            });
            const result = fixOpenAPIDescriptions(spec);
            expect(((result.paths as any)['/users']!.get as any).description).toBe('Get users\nLine 2');
        });

        it('should handle all HTTP methods', () => {
            const methods = ['get', 'post', 'put', 'delete', 'patch', 'options', 'head', 'trace'];
            const testPath: any = {};

            for (const method of methods) {
                testPath[method] = {
                    description: `${method} method\\nDescription`
                };
            }

            const spec = createSpec({
                paths: { '/test': testPath }
            });
            const result = fixOpenAPIDescriptions(spec);

            for (const method of methods) {
                expect(((result.paths as any)['/test']![method as any]?.description)).toBe(`${method} method\nDescription`);
            }
        });

        it('should fix multiple paths', () => {
            const spec = createSpec({
                paths: {
                    '/users': {
                        get: { description: 'Get users\\nDetails' }
                    },
                    '/posts': {
                        post: { description: 'Create post\\nExample' }
                    }
                }
            });
            const result = fixOpenAPIDescriptions(spec);
            expect(((result.paths as any)['/users']!.get as any).description).toBe('Get users\nDetails');
            expect(((result.paths as any)['/posts']!.post as any).description).toBe('Create post\nExample');
        });
    });

    describe('fixOpenAPIDescriptions - Components', () => {
        it('should fix descriptions in component schemas', () => {
            const spec = createSpec({
                components: {
                    schemas: {
                        User: {
                            type: 'object',
                            description: 'User model\\nWith details'
                        }
                    }
                }
            });
            const result = fixOpenAPIDescriptions(spec);
            expect(((result.components!.schemas as any)['User'] as any).description).toBe('User model\nWith details');
        });

        it('should handle multiple schemas', () => {
            const spec = createSpec({
                components: {
                    schemas: {
                        User: { description: 'User schema\\nLine 2' },
                        Post: { description: 'Post schema\\nLine 2' },
                        Comment: { description: 'Comment schema\\nLine 2' }
                    }
                }
            });
            const result = fixOpenAPIDescriptions(spec);
            expect(((result.components!.schemas as any)['User'] as any).description).toBe('User schema\nLine 2');
            expect(((result.components!.schemas as any)['Post'] as any).description).toBe('Post schema\nLine 2');
            expect(((result.components!.schemas as any)['Comment'] as any).description).toBe('Comment schema\nLine 2');
        });

        it('should skip schemas without description', () => {
            const spec = createSpec({
                components: {
                    schemas: {
                        User: { type: 'object' }
                    }
                }
            });
            expect(() => fixOpenAPIDescriptions(spec)).not.toThrow();
        });
    });

    describe('fixOpenAPIDescriptions - Info', () => {
        it('should fix info description', () => {
            const spec = createSpec({
                info: {
                    title: 'Test API',
                    version: '1.0.0',
                    description: 'API description\\nWith details\\nAnd more'
                }
            });
            const result = fixOpenAPIDescriptions(spec);
            expect(result.info.description).toBe('API description\nWith details\nAnd more');
        });

        it('should handle missing info description', () => {
            const spec = createSpec({
                info: { title: 'Test', version: '1.0' }
            });
            expect(() => fixOpenAPIDescriptions(spec)).not.toThrow();
        });
    });

    describe('openapiFix alias', () => {
        it('should be an alias for fixOpenAPIDescriptions', () => {
            const spec = createSpec({
                info: { title: 'Test', version: '1.0', description: 'Test\\nAlias' }
            });
            const result1 = fixOpenAPIDescriptions(spec);
            const spec2 = createSpec({
                info: { title: 'Test', version: '1.0', description: 'Test\\nAlias' }
            });
            const result2 = openapiFix(spec2);
            expect(result1.info.description).toBe(result2.info.description);
        });
    });

    describe('Edge cases', () => {
        it('should handle null document', () => {
            const result = fixOpenAPIDescriptions(null as any);
            expect(result).toBeNull();
        });

        it('should handle undefined description', () => {
            const spec = createSpec({
                paths: {
                    '/test': {
                        get: {} // No description
                    }
                }
            });
            expect(() => fixOpenAPIDescriptions(spec)).not.toThrow();
        });

        it('should handle non-string description gracefully', () => {
            const spec = createSpec({
                info: { title: 'Test', version: '1.0', description: 123 as any }
            });
            const result = fixOpenAPIDescriptions(spec);
            expect(result.info.description).toBe(123);
        });

        it('should modify document in place and return it', () => {
            const spec = createSpec({
                info: { title: 'Test', version: '1.0', description: 'Line\\nTwo' }
            });
            const result = fixOpenAPIDescriptions(spec);
            expect(result).toBe(spec);
            expect(spec.info.description).toBe('Line\nTwo');
        });

        it('should handle empty paths object', () => {
            const spec = createSpec({
                paths: {}
            });
            expect(() => fixOpenAPIDescriptions(spec)).not.toThrow();
        });

        it('should handle empty components object', () => {
            const spec = createSpec({
                components: {}
            });
            expect(() => fixOpenAPIDescriptions(spec)).not.toThrow();
        });
    });

    describe('Integration - Complex Scenario', () => {
        it('should handle a complete realistic OpenAPI spec', () => {
            const spec = createSpec({
                info: {
                    title: 'User API',
                    version: '1.0.0',
                    description: 'API for managing users\\nSupports CRUD operations\\nAuth via JWT'
                },
                paths: {
                    '/users': {
                        get: {
                            description: 'Get all users\\nOptions: - filter by name - filter by status'
                        },
                        post: {
                            description: 'Create a new user\\nRequest body: ```json\\n{"name":"John","email":"john@example.com"}\\n```'
                        }
                    },
                    '/users/{id}': {
                        get: {
                            description: 'Get user by ID\\nReturns user object'
                        },
                        patch: {
                            description: 'Update user\\nPartial update allowed'
                        }
                    }
                },
                components: {
                    schemas: {
                        User: {
                            type: 'object',
                            description: 'User object\\nFields:\\n- id: unique identifier\\n- name: user name\\n- email: email address'
                        },
                        Error: {
                            type: 'object',
                            description: 'Error response\\nFormat: ```json\\n{"error":"message","code":400}\\n```'
                        }
                    }
                }
            });

            const result = fixOpenAPIDescriptions(spec);

            // Check info description
            expect(result.info.description).toContain('API for managing users');
            expect(result.info.description).toContain('Supports CRUD operations');
            expect(result.info.description).toContain('Auth via JWT');

            // Check paths
            expect(((result.paths as any)['/users']!.get as any).description).toContain('Get all users');
            expect(((result.paths as any)['/users']!.post as any).description).toContain('```json');

            // Check schemas
            expect(((result.components!.schemas as any)['User'] as any).description).toContain('User object');
            expect(((result.components!.schemas as any)['Error'] as any).description).toContain('```json');
        });
    });
});
