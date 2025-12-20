// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

/**
 * OpenAPI Code Snippets Generator
 * 
 * This module generates code snippets in multiple programming languages
 * for each endpoint in an OpenAPI specification and adds them as x-code-samples
 * extensions to the endpoint documentation.
 * 
 * @module openapi-snippets
 * @version 1.0.0
 */

import { HTTPSnippet } from './httpsnippet';
import { getAll } from './openapi-to-har';
import type { HarRequest } from './httpsnippet';

/**
 * Code sample extension for OpenAPI endpoints
 * 
 * The x-code-samples extension is a vendor extension that provides
 * executable code examples in various programming languages for an API endpoint.
 * 
 * @typedef {Object} CodeSample
 * @property {string} lang - Programming language identifier (e.g., 'python', 'javascript')
 * @property {string} label - Human-readable label for the code sample (e.g., 'Python (requests)', 'JavaScript (fetch)')
 * @property {string} source - The actual executable source code
 */
interface CodeSample {
    lang: string;
    label: string;
    source: string;
}

/**
 * OpenAPI Operation object
 * 
 * Represents a single HTTP operation (GET, POST, etc.) on an endpoint path.
 * 
 * @typedef {Object} OpenAPIOperation
 * @property {string} [summary] - Short description of the operation
 * @property {string} [description] - Long description of the operation
 * @property {Array} [parameters] - Query, path, header, and cookie parameters
 * @property {Object} [requestBody] - Request body schema and examples
 * @property {Object} responses - Expected response codes and schemas
 * @property {Array<CodeSample>} [x-code-samples] - Generated code samples (vendor extension)
 * @property {any} [key: string] - Any other OpenAPI properties
 */
interface OpenAPIOperation {
    summary?: string;
    description?: string;
    parameters?: any[];
    requestBody?: any;
    responses: any;
    'x-code-samples'?: CodeSample[];
    [key: string]: any;
}

/**
 * OpenAPI Path Item object
 * 
 * Represents all operations available on a single API endpoint path.
 * 
 * @typedef {Object} OpenAPIPathItem
 * @property {OpenAPIOperation} [get] - GET operation on this path
 * @property {OpenAPIOperation} [post] - POST operation on this path
 * @property {OpenAPIOperation} [put] - PUT operation on this path
 * @property {OpenAPIOperation} [patch] - PATCH operation on this path
 * @property {OpenAPIOperation} [delete] - DELETE operation on this path
 * @property {OpenAPIOperation} [options] - OPTIONS operation on this path
 * @property {OpenAPIOperation} [head] - HEAD operation on this path
 * @property {OpenAPIOperation} [trace] - TRACE operation on this path
 * @property {any} [key: string] - Any other properties
 */
interface OpenAPIPathItem {
    get?: OpenAPIOperation;
    post?: OpenAPIOperation;
    put?: OpenAPIOperation;
    patch?: OpenAPIOperation;
    delete?: OpenAPIOperation;
    options?: OpenAPIOperation;
    head?: OpenAPIOperation;
    trace?: OpenAPIOperation;
    [key: string]: any;
}

/**
 * OpenAPI Document structure
 * 
 * @typedef {Object} OpenAPIDocument
 * @property {string} openapi - OpenAPI specification version (e.g., "3.0.0")
 * @property {Object} info - API metadata (title, version, etc.)
 * @property {Object<string, OpenAPIPathItem>} paths - All API endpoints
 * @property {any} [components] - Reusable components (schemas, security schemes, etc.)
 * @property {string} [host] - API server host (optional)
 * @property {string} [basePath] - Base path for all endpoints (optional)
 * @property {any} [key: string] - Any other OpenAPI properties
 */
interface OpenAPIDocument {
    openapi: string;
    info: {
        title: string;
        version: string;
        [key: string]: any;
    };
    paths: {
        [path: string]: OpenAPIPathItem;
    };
    components?: any;
    host?: string;
    basePath?: string;
    [key: string]: any;
}

/**
 * HTTP methods that can have an operation in an OpenAPI path
 * @type {Array<string>}
 * @const
 */
const HTTP_METHODS = ['get', 'post', 'put', 'patch', 'delete', 'options', 'head', 'trace'] as const;

/**
 * Target language interface for code generation
 * 
 * @typedef {Object} TargetLanguage
 * @property {string} language - Programming language identifier (used by HTTPSnippet)
 * @property {string} client - HTTP client library for the language
 * @property {string} [label] - Optional display label for the language (defaults to "{language} ({client})")
 */
interface TargetLanguage {
    language: string;
    client: string;
    label?: string;
}

/**
 * Add code snippets to all endpoints in an OpenAPI document
 * 
 * This function iterates through all endpoints and HTTP methods in the OpenAPI specification,
 * generates code snippets for each target language/client combination, and adds them as
 * x-code-samples vendor extensions to each operation.
 * 
 * The x-code-samples extension is recognized by tools like Swagger UI, RapiDoc, and other
 * API documentation generators to display code examples.
 * 
 * @param {OpenAPIDocument} data - The OpenAPI specification document
 * @param {Array<TargetLanguage>} targetLanguages - Array of target languages and client libraries
 * @returns {OpenAPIDocument} The same OpenAPI document with x-code-samples added to each operation
 * @throws {Error} Re-throws errors from code generation (original document may be partially modified)
 * 
 * @example
 * const spec = {
 *   openapi: '3.0.0',
 *   info: { title: 'My API', version: '1.0.0' },
 *   paths: {
 *     '/users': {
 *       get: {
 *         summary: 'List users',
 *         responses: { '200': { description: 'Success' } }
 *       }
 *     }
 *   }
 * };
 * 
 * const targets = [
 *   { language: 'python', client: 'requests' },
 *   { language: 'javascript', client: 'fetch' }
 * ];
 * 
 * openapiAddSnippets(spec, targets);
 * // spec.paths['/users'].get['x-code-samples'] will now contain Python and JavaScript code
 */
export function openapiAddSnippets(
    data: OpenAPIDocument,
    targetLanguages: TargetLanguage[]
): OpenAPIDocument {
    /**
     * Validate input parameters
     */
    if (!data || !data.paths) {
        console.warn('[openapi-snippets] Invalid OpenAPI document: missing paths');
        return data;
    }

    if (!targetLanguages || targetLanguages.length === 0) {
        console.warn('[openapi-snippets] No target languages specified');
        return data;
    }

    /**
     * Get all endpoints with HAR format from the OpenAPI spec
     * 
     * The getAll() function converts the OpenAPI specification into
     * an array of endpoints with HAR-format request examples.
     */
    let endpoints;
    try {
        endpoints = getAll(data);
    } catch (err) {
        console.error('[openapi-snippets] Failed to extract endpoints from OpenAPI spec:', err);
        return data;
    }

    /**
     * Create a map of endpoints for quick lookup
     * 
     * Map key format: "{method} {path}"
     * This allows us to quickly find the OpenAPI operation object to add code samples to.
     * 
     * @type {Map<string, {path: string, method: string, operation: OpenAPIOperation}>}
     */
    const endpointMap = new Map<string, {
        path: string;
        method: string;
        operation: OpenAPIOperation;
    }>();

    // Build the map from the OpenAPI paths
    for (const [path, pathItem] of Object.entries(data.paths)) {
        for (const method of HTTP_METHODS) {
            if (pathItem[method as keyof OpenAPIPathItem]) {
                const operation = pathItem[method as keyof OpenAPIPathItem] as OpenAPIOperation;
                const key = `${method.toUpperCase()} ${path}`;
                endpointMap.set(key, { path, method, operation });
            }
        }
    }

    /**
     * For each extracted endpoint, generate code snippets
     */
    for (const endpoint of endpoints) {
        /**
         * Extract the path from the endpoint URL
         * 
         * endpoint.url might be a full URL like "https://api.example.com/api/stream/stats"
         * We need to extract just the path part "/api/stream/stats" to match against
         * the OpenAPI paths object.
         */
        let endpointPath: string;
        try {
            // Try to parse as a full URL
            const url = new URL(endpoint.url);
            endpointPath = url.pathname;
        } catch {
            // If it's not a valid URL, assume it's already just the path
            endpointPath = endpoint.url;
        }

        /**
         * Extract HAR requests from the endpoint
         * 
         * Each endpoint can have multiple example requests (in HAR format).
         * We'll generate code snippets for each one.
         */
        const hars = endpoint.hars || [];

        if (hars.length === 0) {
            console.warn(`[openapi-snippets] No HAR requests found for ${endpoint.method} ${endpointPath}`);
            continue;
        }

        /**
         * Use the first HAR request as the representative example
         * 
         * In practice, most endpoints have one main example.
         * We could extend this to generate snippets for all examples if needed.
         */
        const harRequest = hars[0];

        /**
         * Find the corresponding OpenAPI operation object using the extracted path
         */
        const key = `${endpoint.method.toUpperCase()} ${endpointPath}`;
        const endpointInfo = endpointMap.get(key);

        if (!endpointInfo) {
            console.warn(`[openapi-snippets] Could not find OpenAPI operation for ${key}`);
            console.warn(`[openapi-snippets] Available keys: ${Array.from(endpointMap.keys()).join(', ')}`);
            continue;
        }

        const { operation } = endpointInfo;

        /**
         * Initialize the x-code-samples array if it doesn't exist
         */
        if (!operation['x-code-samples']) {
            operation['x-code-samples'] = [];
        }

        /**
         * Generate code snippets for each target language
         */
        for (const target of targetLanguages) {
            try {
                /**
                 * Create HTTPSnippet instance for this request
                 */
                const httpSnippet = new HTTPSnippet(harRequest as any);

                /**
                 * Generate code snippet for the target language and client
                 */
                const source = httpSnippet.convert(target.language, target.client);

                /**
                 * Skip if code generation returned false (error)
                 */
                if (source === false) {
                    console.warn(
                        `[openapi-snippets] Failed to generate ${target.language}/${target.client} ` +
                        `snippet for ${endpoint.method} ${endpoint.url}`
                    );
                    continue;
                }

                /**
                 * Handle both string and array results
                 * 
                 * Some code generators return an array of strings (one for each request).
                 * We join them with newlines for clarity.
                 */
                const sourceCode = Array.isArray(source) ? source.join('\n') : source;

                /**
                 * Create the code sample entry
                 * 
                 * The label combines the language and client for clarity.
                 * For example: "Python (requests)", "JavaScript (fetch)"
                 */
                const label = target.label || `${target.language} (${target.client})`;

                const codeSample: CodeSample = {
                    lang: target.language,
                    label,
                    source: sourceCode
                };

                /**
                 * Add the code sample to the operation
                 */
                operation['x-code-samples']!.push(codeSample);

                // console.log(
                //     `[openapi-snippets] Generated ${target.language}/${target.client} ` +
                //     `snippet for ${endpoint.method} ${endpoint.url}`
                // );
            } catch (err) {
                console.error(
                    `[openapi-snippets] Error generating ${target.language}/${target.client} ` +
                    `snippet for ${endpoint.method} ${endpoint.url}:`,
                    err
                );
                // Continue with next language rather than stopping
            }
        }
    }

    return data;
}

/**
 * Alternative function name for consistency with common naming patterns
 * 
 * This is an alias for openapiAddSnippets that follows the pattern of
 * enhancing/augmenting OpenAPI documents.
 * 
 * @type {typeof openapiAddSnippets}
 */
export const augmentOpenAPIWithSnippets = openapiAddSnippets;

/**
 * Generic function to add code snippets to any OpenAPI-like document
 * 
 * This is a type-safe wrapper that accepts a generic OpenAPI document type
 * and returns the same type with code snippets added. This allows proper
 * type inference and ensures type safety throughout the application.
 * 
 * @template T - The OpenAPI document type (must have paths property)
 * @param {T} data - The OpenAPI specification document
 * @param {Array<TargetLanguage>} targetLanguages - Array of target languages and client libraries
 * @returns {T} The same OpenAPI document type with x-code-samples added to each operation
 * 
 * @example
 * type OpenAPI3 = { openapi: string; info: any; paths: any; components?: any };
 * const spec: OpenAPI3 = { ... };
 * const result: OpenAPI3 = openapiAddSnippetsGeneric(spec, targets);
 */
export function openapiAddSnippetsGeneric<T extends { paths: { [key: string]: any } }>(
    data: T,
    targetLanguages: TargetLanguage[]
): T {
    /**
     * Cast to OpenAPIDocument for internal processing
     * 
     * Since we only care about the 'paths' property and the structure is compatible,
     * we can safely cast the generic document to our OpenAPIDocument type.
     */
    const document = data as unknown as OpenAPIDocument;

    /**
     * Call the main function with the cast document
     */
    openapiAddSnippets(document, targetLanguages);

    /**
     * Return the original type to maintain type safety
     */
    return data;
}
