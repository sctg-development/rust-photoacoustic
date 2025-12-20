// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

/**
 * HTTPSnippet - Browser Compatible Code Snippet Generator
 * 
 * This module provides functionality to generate executable code snippets
 * in multiple programming languages (Python, JavaScript, Go, Rust, C, PHP, Java, etc.)
 * from HTTP request information stored in the HAR (HTTP Archive) format.
 * 
 * Key Features:
 * - Convert HAR requests to executable code in 20+ programming languages
 * - Support for various HTTP client libraries (requests, fetch, axios, etc.)
 * - Browser-compatible implementation without Node.js dependencies
 * - Automatic URL parsing and query parameter handling
 * - Support for multiple content types (JSON, form-data, multipart, etc.)
 * 
 * @module httpsnippet
 * @version 1.0.0
 * @example
 * ```typescript
 * const harRequest = {
 *   method: 'GET',
 *   url: 'https://api.example.com/users',
 *   headers: [{name: 'Authorization', value: 'Bearer token'}],
 *   queryString: [],
 *   httpVersion: 'HTTP/1.1',
 *   cookies: [],
 *   headersSize: 0,
 *   bodySize: 0
 * };
 * 
 * const snippet = new HTTPSnippet(harRequest);
 * const pythonCode = snippet.convert('python', 'requests');
 * console.log(pythonCode);
 * ```
 */

// Import types and utility functions for request processing
import type { ReducedHelperObject } from './helpers/reducer';
import { reducer } from './helpers/reducer';
import { getHeaderName } from './helpers/headers';
import * as targets from './targets/targets';

/**
 * HAR Parameter Object
 * 
 * Represents a single key-value pair used in HTTP requests,
 * such as query parameters, headers, or form fields.
 * 
 * @typedef {Object} HarParameter
 * @property {string} name - The parameter name or key (e.g., 'Authorization', 'page')
 * @property {string} value - The parameter value (e.g., 'Bearer token123', '1')
 * @property {string} [fileName] - Optional filename for file uploads in multipart requests
 * @property {string} [contentType] - Optional MIME type for file uploads (e.g., 'image/png')
 * 
 * @example
 * // Query parameter
 * {name: 'page', value: '1'}
 * 
 * // HTTP header
 * {name: 'Content-Type', value: 'application/json'}
 * 
 * // File upload
 * {name: 'file', value: 'fileContent', fileName: 'upload.txt', contentType: 'text/plain'}
 */
export interface HarParameter {
    name: string;
    value: string;
    fileName?: string;
    contentType?: string;
}

/**
 * HTTP Request Body Data
 * 
 * Represents the body of an HTTP request with its content type
 * and either text content or structured form parameters.
 * 
 * @typedef {Object} HarPostData
 * @property {string} mimeType - Content type of the body (e.g., 'application/json', 'multipart/form-data')
 * @property {string} [text] - Raw text body content (used for JSON, XML, plain text)
 * @property {HarParameter[]} [params] - Structured form parameters (used for form submissions)
 * @property {string} [comment] - Optional comment describing the request body
 * 
 * @example
 * // JSON body
 * {
 *   mimeType: 'application/json',
 *   text: '{"name": "John", "age": 30}'
 * }
 * 
 * // Form data
 * {
 *   mimeType: 'application/x-www-form-urlencoded',
 *   params: [
 *     {name: 'username', value: 'john'},
 *     {name: 'password', value: 'secret'}
 *   ]
 * }
 */
export interface HarPostData {
    mimeType: string;
    text?: string;
    params?: HarParameter[];
    comment?: string;
}

/**
 * HTTP Request Object (HAR Format)
 * 
 * Represents a complete HTTP request in the HAR (HTTP Archive) specification format.
 * HAR is a standard format used to record HTTP transactions for debugging and testing.
 * 
 * See: http://www.softwareishard.com/blog/har-12-spec/#request
 * 
 * @typedef {Object} HarRequest
 * @property {string} method - HTTP method (GET, POST, PUT, DELETE, PATCH, HEAD, OPTIONS)
 * @property {string} url - Complete request URL (e.g., 'https://api.example.com/users?page=1')
 * @property {HarParameter[]} headers - HTTP headers array
 * @property {HarParameter[]} queryString - Query string parameters array
 * @property {string} httpVersion - HTTP version (e.g., 'HTTP/1.1', 'HTTP/2')
 * @property {HarParameter[]} cookies - HTTP cookies array
 * @property {number} headersSize - Total size of HTTP headers in bytes (for statistics)
 * @property {number} bodySize - Total size of request body in bytes (for statistics)
 * @property {HarPostData} [postData] - Request body information
 * @property {string} [comment] - Optional comment or description
 * 
 * @example
 * ```typescript
 * const request: HarRequest = {
 *   method: 'POST',
 *   url: 'https://api.example.com/users?active=true',
 *   headers: [
 *     {name: 'Content-Type', value: 'application/json'},
 *     {name: 'Authorization', value: 'Bearer token123'}
 *   ],
 *   queryString: [{name: 'active', value: 'true'}],
 *   httpVersion: 'HTTP/1.1',
 *   cookies: [{name: 'sessionId', value: 'abc123'}],
 *   headersSize: 142,
 *   bodySize: 256,
 *   postData: {
 *     mimeType: 'application/json',
 *     text: '{"name": "Alice", "email": "alice@example.com"}'
 *   }
 * };
 * ```
 */
export interface HarRequest {
    method: string;
    url: string;
    headers: HarParameter[];
    queryString: HarParameter[];
    httpVersion: string;
    cookies: HarParameter[];
    headersSize: number;
    bodySize: number;
    postData?: HarPostData;
    comment?: string;
}

/**
 * HAR Log Entry
 * 
 * A container object representing one or more HTTP transactions
 * recorded in the HAR (HTTP Archive) format.
 * 
 * The HAR format is a standard JSON format for recording HTTP transactions.
 * Multiple requests can be recorded in a single HAR log.
 * 
 * @typedef {Object} HarEntry
 * @property {Object} log - The HAR log object
 * @property {string} log.version - HAR specification version (typically '1.2')
 * @property {Object} log.creator - Information about the tool that created the HAR
 * @property {string} log.creator.name - Name of the HAR creator tool
 * @property {string} log.creator.version - Version of the creator tool
 * @property {Array} log.entries - Array of HTTP request/response pairs
 * 
 * @example
 * ```typescript
 * const harEntry: HarEntry = {
 *   log: {
 *     version: '1.2',
 *     creator: {name: 'httpsnippet', version: '1.0.0'},
 *     entries: [
 *       {request: {...}},
 *       {request: {...}}
 *     ]
 *   }
 * };
 * ```
 */
export interface HarEntry {
    log: {
        version: string;
        creator: {
            name: string;
            version: string;
        };
        entries: Array<{
            request: Partial<HarRequest>;
        }>;
    };
}

/**
 * Processed Request Metadata
 * 
 * These are additional properties added to a HAR request after processing.
 * They contain parsed and normalized versions of the request data that make
 * code generation easier.
 * 
 * @typedef {Object} RequestExtras
 * @property {HarPostData & Object} postData - Processed request body with parsed data
 * @property {string} fullUrl - Complete URL with query parameters
 * @property {ReducedHelperObject} queryObj - Parsed query parameters as key-value pairs
 * @property {ReducedHelperObject} headersObj - Parsed headers as key-value pairs
 * @property {Object} uriObj - Parsed URL components (protocol, hostname, pathname, etc.)
 * @property {ReducedHelperObject} cookiesObj - Parsed cookies as key-value pairs
 * @property {ReducedHelperObject} allHeaders - All headers combined, including cookies
 */
export interface RequestExtras {
    postData: HarPostData & {
        jsonObj?: Record<string, any>;
        paramsObj?: ReducedHelperObject;
        boundary?: string;
    };
    fullUrl: string;
    queryObj: ReducedHelperObject;
    headersObj: ReducedHelperObject;
    uriObj: any;
    cookiesObj: ReducedHelperObject;
    allHeaders: ReducedHelperObject;
}

/**
 * Complete Request Object
 * 
 * A HAR request combined with the processed metadata.
 * This is used internally by HTTPSnippet for code generation.
 * 
 * @typedef {HarRequest & RequestExtras} Request
 */
export type Request = HarRequest & RequestExtras;

/**
 * Type guard to check if a value is a HarEntry
 * 
 * This function uses TypeScript type narrowing to safely determine
 * if an object is a HarEntry (containing multiple requests) or
 * a single HarRequest object.
 * 
 * Type guards are useful because:
 * 1. They help with type safety at runtime
 * 2. They allow TypeScript to narrow the type for better IDE support
 * 3. They prevent runtime errors from incorrect type assumptions
 * 
 * @param {any} value - The value to check
 * @returns {boolean} True if value is a HarEntry, false otherwise
 * 
 * @example
 * ```typescript
 * const data = await fetch('/api.json').then(r => r.json());
 * 
 * if (isHarEntry(data)) {
 *   console.log('This is a HAR log with', data.log.entries.length, 'requests');
 * } else {
 *   console.log('This is a single request');
 * }
 * ```
 */
export const isHarEntry = (value: any): value is HarEntry =>
    value !== null &&
    typeof value === 'object' &&
    'log' in value &&
    typeof value.log === 'object' &&
    'entries' in value.log &&
    Array.isArray(value.log.entries);

/**
 * Parse a URL string into its component parts
 * 
 * This function breaks down a URL into logical components:
 * - protocol: 'https:'
 * - hostname: 'api.example.com'
 * - port: '8080'
 * - pathname: '/api/v1/users'
 * - search: '?page=1&limit=10'
 * - query: {page: '1', limit: '10'} (parsed from search)
 * 
 * If the URL is invalid, the function returns a fallback object
 * to prevent parsing errors from crashing the application.
 * 
 * @param {string} urlString - The URL to parse (e.g., 'https://api.example.com/path?query=value')
 * @returns {Object} Object with URL components:
 *   - protocol: URL scheme ('https:', 'http:', etc.)
 *   - hostname: Domain name
 *   - port: Port number (if specified)
 *   - pathname: Path component
 *   - search: Query string with leading '?'
 *   - hash: Fragment identifier with leading '#'
 *   - href: Complete URL
 *   - query: Parsed query parameters as an object
 * 
 * @example
 * const parts = parseUrl('https://api.example.com:8080/users?page=1&sort=name');
 * console.log(parts.hostname);  // 'api.example.com'
 * console.log(parts.port);      // '8080'
 * console.log(parts.pathname);  // '/users'
 * console.log(parts.query);     // {page: '1', sort: 'name'}
 */
function parseUrl(urlString: string): any {
    try {
        const url = new URL(urlString);
        return {
            protocol: url.protocol,
            hostname: url.hostname,
            port: url.port,
            pathname: url.pathname,
            search: url.search,
            hash: url.hash,
            href: url.href,
            host: url.host,
            query: Object.fromEntries(url.searchParams),
        };
    } catch (e) {
        return {
            href: urlString,
            pathname: urlString,
            query: {},
        };
    }
}

/**
 * Reconstruct a URL from its parsed components
 * 
 * This is the inverse of parseUrl. Given URL components,
 * it constructs a complete URL string.
 * 
 * The function handles:
 * - Combining protocol, hostname, port into the authority
 * - Appending the path (pathname)
 * - Adding query string (search)
 * - Adding fragment identifier (hash)
 * 
 * @param {Object} parts - URL components object with optional properties:
 *   - protocol: URL scheme (e.g., 'https:')
 *   - hostname: Domain name
 *   - port: Port number
 *   - pathname: Path component (e.g., '/api/users')
 *   - search: Query string with leading '?' (e.g., '?page=1')
 *   - hash: Fragment with leading '#' (e.g., '#section')
 *   - href: If provided, returned as-is
 * @returns {string} Complete URL string
 * 
 * @example
 * const url = formatUrl({
 *   protocol: 'https:',
 *   hostname: 'api.example.com',
 *   port: '8080',
 *   pathname: '/users',
 *   search: '?page=1'
 * });
 * console.log(url); // 'https://api.example.com:8080/users?page=1'
 */
function formatUrl(parts: any): string {
    if (typeof parts === 'string') return parts;
    if (parts.href) return parts.href;

    let url = '';
    if (parts.protocol) url += parts.protocol;
    if (parts.hostname) {
        url += '//';
        if (parts.hostname) url += parts.hostname;
        if (parts.port) url += `:${parts.port}`;
    }
    if (parts.pathname) url += parts.pathname;
    if (parts.search) url += parts.search;
    if (parts.hash) url += parts.hash;

    return url;
}

/**
 * Convert an object of query parameters into a URL query string
 * 
 * This function encodes parameters for safe use in URLs.
 * It handles:
 * - Multiple values for the same parameter (arrays)
 * - Special characters that need URL encoding
 * - Proper formatting with & separators
 * 
 * URL encoding is important because URLs can only contain certain characters.
 * Special characters like spaces are encoded as %20, & becomes %26, etc.
 * 
 * @param {Object} params - Query parameters object (key-value pairs or arrays)
 * @returns {string} Query string without the leading '?' (e.g., 'page=1&limit=10&tags=a&tags=b')
 * 
 * @example
 * const queryString = stringifyQuery({
 *   page: '1',
 *   limit: '10',
 *   tags: ['javascript', 'web']
 * });
 * console.log(queryString);
 * // 'page=1&limit=10&tags=javascript&tags=web'
 */
function stringifyQuery(params: Record<string, any>): string {
    if (!params || typeof params !== 'object') return '';

    return Object.entries(params)
        .map(([key, value]) => {
            if (Array.isArray(value)) {
                // For array parameters, create multiple key=value pairs
                return value.map((v) => `${encodeURIComponent(key)}=${encodeURIComponent(v)}`).join('&');
            }
            // Single value parameter
            return `${encodeURIComponent(key)}=${encodeURIComponent(value)}`;
        })
        .join('&');
}

/**
 * HTTPSnippet Class - Main Code Generator
 * 
 * This class is the core of the library. It takes HTTP request information
 * (in HAR format) and generates executable code snippets in multiple
 * programming languages.
 * 
 * How it works:
 * 1. Constructor accepts a HAR request or HAR log entry
 * 2. Requests are normalized and prepared (headers parsed, URLs cleaned, etc.)
 * 3. The convert() method generates code in the requested language
 * 
 * @example
 * ```typescript
 * // Create from a single HAR request
 * const snippet = new HTTPSnippet({
 *   method: 'GET',
 *   url: 'https://api.example.com/users',
 *   headers: [],
 *   queryString: [],
 *   httpVersion: 'HTTP/1.1',
 *   cookies: [],
 *   headersSize: 0,
 *   bodySize: 0
 * });
 * 
 * // Generate Python code using the 'requests' library
 * const pythonCode = snippet.convert('python', 'requests');
 * console.log(pythonCode);
 * // Output:
 * // import requests
 * // response = requests.get('https://api.example.com/users')
 * 
 * // Generate JavaScript code using fetch
 * const jsCode = snippet.convert('javascript', 'fetch');
 * console.log(jsCode);
 * // Output:
 * // fetch('https://api.example.com/users')
 * //   .then(response => response.json())
 * ```
 */
export class HTTPSnippet {
    /**
     * Array of processed HTTP requests
     * 
     * Each request is normalized and prepared for code generation.
     * The array typically contains one request, but can contain multiple
     * if created from a HAR log entry.
     * 
     * @type {Request[]}
     */
    requests: Request[] = [];

    /**
     * Constructor - Initialize HTTPSnippet with HAR request(s)
     * 
     * Accepts either:
     * - A single HAR request object
     * - A HAR log entry (contains multiple requests)
     * 
     * The constructor normalizes the input and prepares requests for code generation.
     * 
     * @param {HarEntry | HarRequest} input - Either a HarEntry or a single HarRequest
     * 
     * @example
     * // Single request
     * const snippet1 = new HTTPSnippet(harRequest);
     * 
     * // Multiple requests from a HAR file
     * const snippet2 = new HTTPSnippet(harEntry);
     */
    constructor(input: HarEntry | HarRequest) {
        let entries: Array<{ request: Partial<HarRequest> }> = [];

        // Determine if input is a HAR entry (multiple requests) or a single request
        if (isHarEntry(input)) {
            entries = input.log.entries;
        } else {
            // Wrap single request in an entry structure
            entries = [{ request: input }];
        }

        this.requests = [];

        // Process each request entry
        entries.forEach(({ request }) => {
            // Create a complete request object with sensible defaults
            const req = {
                bodySize: 0,
                headersSize: 0,
                headers: [],
                cookies: [],
                httpVersion: 'HTTP/1.1',
                queryString: [],
                ...request,
                postData: request?.postData || {
                    mimeType: request.postData?.mimeType || 'application/octet-stream',
                },
            } as HarRequest;

            /**
             * Normalize URL by replacing OpenAPI path parameters
             * 
             * OpenAPI uses curly braces for path parameters (e.g., /users/{id}).
             * These are replaced with double underscores so URLs can be properly parsed.
             * This is a workaround because curly braces aren't valid in URLs.
             */
            if (req.url && (req.url.includes('{') || req.url.includes('}'))) {
                req.url = req.url.replace(/{/g, '__').replace(/}/g, '__');
            }

            // Only add valid requests (must have method and URL)
            if (req.method && req.url) {
                this.requests.push(this.prepare(req));
            }
        });
    }

    /**
     * Prepare a HAR request for code generation
     * 
     * This method normalizes and processes a request by:
     * 1. Parsing and merging query parameters
     * 2. Converting headers to an object for easy access
     * 3. Converting cookies to an object and creating a Cookie header
     * 4. Processing request body based on content type
     * 5. Parsing the URL and merging URL query parameters with declared ones
     * 6. Building the full URL with query string
     * 
     * After this processing, the request is ready for code generation.
     * Code generators can easily access headers, cookies, query parameters, etc.
     * 
     * @param {HarRequest} harRequest - The HAR request to prepare
     * @returns {Request} The prepared request with processed metadata
     * 
     * @private
     */
    private prepare(harRequest: HarRequest): Request {
        const request: Request = {
            ...harRequest,
            fullUrl: '',
            uriObj: {},
            queryObj: {},
            headersObj: {},
            cookiesObj: {},
            allHeaders: {},
            postData: harRequest.postData || { mimeType: 'application/octet-stream' },
        };

        /**
         * Process query string parameters
         * 
         * Query parameters are extracted from the queryString array
         * and converted to an object (key-value pairs) using the reducer function.
         * This makes it easier to generate code because we have key-value access
         * instead of needing to iterate through arrays.
         * 
         * Example: [{name: 'page', value: '1'}, {name: 'limit', value: '10'}]
         * Becomes: {page: '1', limit: '10'}
         */
        if (request.queryString && request.queryString.length) {
            request.queryObj = request.queryString.reduce(reducer, {});
        }

        /**
         * Process HTTP headers
         * 
         * Headers are converted from an array to an object,
         * with header names converted to lowercase for case-insensitive access.
         * This is important because HTTP headers are case-insensitive.
         */
        if (request.headers && request.headers.length) {
            request.headersObj = request.headers.reduce(
                (acc: ReducedHelperObject, { name, value }) => {
                    // Header names are case-insensitive in HTTP, so use lowercase
                    acc[name.toLowerCase()] = value;
                    return acc;
                },
                {}
            );
        }

        /**
         * Process cookies
         * 
         * Cookies are converted from an array to an object,
         * and also formatted as a proper Cookie header value.
         * 
         * The Cookie header value is a special format:
         * Cookie: name1=value1; name2=value2; name3=value3
         * 
         * Note: We use reduceRight instead of reduce to maintain cookie order
         * in case there are multiple values for the same cookie name.
         */
        if (request.cookies && request.cookies.length) {
            request.cookiesObj = request.cookies.reduceRight(
                (acc: ReducedHelperObject, { name, value }) => {
                    acc[name] = value;
                    return acc;
                },
                {}
            );

            /**
             * Format cookies as HTTP Cookie header
             * 
             * The Cookie header format is:
             * Cookie: sessionId=abc123; theme=dark; preferences=json%7Btype%3A%22strict%22%7D
             * 
             * Values are URL-encoded for safety.
             */
            const cookieHeader = request.cookies
                .map(({ name, value }) => `${encodeURIComponent(name)}=${encodeURIComponent(value)}`)
                .join('; ');

            if (cookieHeader) {
                request.allHeaders.cookie = cookieHeader;
            }
        }

        /**
         * Process request body (post data)
         * 
         * The processing depends on the content type:
         * - JSON: Parse JSON text to create a jsonObj for reference
         * - Form-encoded: Parse parameters and generate URL-encoded text
         * - Multipart: Prepare parameters for multipart boundary generation
         * - Other: Leave as raw text
         */
        const postData = request.postData;

        switch (postData.mimeType) {
            // JSON content types
            case 'application/json':
            case 'text/json':
            case 'text/x-json':
            case 'application/x-json':
                // Normalize MIME type
                postData.mimeType = 'application/json';
                // Parse JSON for code generators that need the parsed object
                if (postData.text) {
                    try {
                        postData.jsonObj = JSON.parse(postData.text);
                    } catch (e) {
                        // If JSON parsing fails, treat as plain text
                        postData.mimeType = 'text/plain';
                    }
                }
                break;

            // Form-encoded parameters
            case 'application/x-www-form-urlencoded':
                // Convert parameter array to object and then to URL-encoded string
                if (postData.params) {
                    postData.paramsObj = postData.params.reduce(reducer, {});
                    // URL-encode the parameters
                    postData.text = stringifyQuery(postData.paramsObj as any);
                }
                break;

            // Multipart form data
            case 'multipart/form-data':
                // Ensure text field exists (will be populated with boundary when needed)
                if (!postData.params) {
                    postData.text = '';
                }
                postData.mimeType = 'multipart/form-data';
                break;
        }

        /**
         * Merge all headers
         * 
         * We combine cookies (as the Cookie header) with other headers.
         * Headers from request.headersObj take precedence over allHeaders.
         */
        const allHeaders = {
            ...request.allHeaders,
            ...request.headersObj,
        };

        /**
         * Parse the URL
         * 
         * We break down the URL into components (protocol, hostname, pathname, etc.)
         * This makes it easier to reconstruct the URL later with normalized values.
         */
        const urlParts = parseUrl(request.url);

        /**
         * Merge query strings
         * 
         * Query parameters can come from two places:
         * 1. The queryString array in the HAR request
         * 2. The URL itself (e.g., https://api.example.com/users?page=1)
         * 
         * We merge both, with the explicit queryString taking precedence.
         */
        const mergedQuery = {
            ...request.queryObj,
            ...(urlParts.query || {}),
        };

        /**
         * Build the full URL with query string
         * 
         * fullUrl includes the complete URL with all query parameters
         * This is what gets used in the generated code.
         */
        const fullUrlString = formatUrl(urlParts);
        const queryString = stringifyQuery(mergedQuery);
        const fullUrl = queryString ? `${fullUrlString}?${queryString}` : fullUrlString;

        return {
            ...request,
            allHeaders,
            fullUrl,
            url: formatUrl(urlParts),
            uriObj: urlParts,
            queryObj: mergedQuery,
        };
    }

    /**
     * Generate code snippet for the stored request(s)
     * 
     * This is the main public method that generates executable code.
     * It selects a target language (Python, JavaScript, etc.) and
     * a client library for that language (requests, axios, fetch, etc.).
     * 
     * The method finds the appropriate code generator for the target/client combination
     * and uses it to convert the HAR request into actual code.
     * 
     * Supported targets include:
     * - 'shell': Bash/sh shell (curl, httpie, wget)
     * - 'javascript': Node.js and browser JavaScript (axios, fetch, xhr)
     * - 'python': Python 2 and 3 (requests, urllib, httpx)
     * - 'rust': Rust (reqwest)
     * - 'go': Go (net/http)
     * - 'java': Java (okhttp, httpclient, etc.)
     * - 'csharp': C# (.NET HttpClient)
     * - 'php': PHP (curl, guzzle)
     * - 'ruby': Ruby (net/http, faraday)
     * - 'swift': Swift (URLSession)
     * - 'objc': Objective-C (NSURLSession)
     * - 'c': C (libcurl)
     * - And many more...
     * 
     * @param {string} targetId - Programming language/platform identifier
     *   Examples: 'python', 'javascript', 'rust', 'go', 'shell', 'java'
     * @param {string} [clientId] - Specific HTTP client library for the target
     *   If not provided, uses the target's default client
     *   Examples: 'requests' (for Python), 'axios' (for JavaScript), 'curl' (for shell)
     * @param {Object} [options] - Target-specific code generation options
     *   These vary by target. For example, for shell targets:
     *   - indent: indentation string (default: '  ')
     *   - short: use short curl flags (default: false)
     * @returns {string | string[] | false} Generated code snippet
     *   - Single string if only one request
     *   - Array of strings if multiple requests
     *   - false if the target/client combination is not supported
     * 
     * @example
     * ```typescript
     * const snippet = new HTTPSnippet(harRequest);
     * 
     * // Generate Python code (will use default client 'requests')
     * const pythonCode = snippet.convert('python');
     * console.log(pythonCode);
     * // Output: import requests\nresponse = requests.get(...)
     * 
     * // Generate JavaScript with explicit client
     * const jsCode = snippet.convert('javascript', 'fetch');
     * console.log(jsCode);
     * // Output: fetch(...).then(response => response.json())
     * 
     * // Generate shell script with custom options
     * const shellCode = snippet.convert('shell', 'curl', {
     *   indent: '    ',
     *   short: true
     * });
     * 
     * // Handle multiple requests
     * const harEntry = {log: {version: '1.2', ...}};
     * const multiSnippet = new HTTPSnippet(harEntry);
     * const codes = multiSnippet.convert('python', 'requests');
     * if (Array.isArray(codes)) {
     *   codes.forEach((code, i) => console.log(`Request ${i + 1}:\\n${code}`));
     * }
     * ```
     */
    convert(
        targetId: string,
        clientId?: string,
        options?: any
    ): string | string[] | false {
        try {
            /**
             * Look up the target language implementation
             * 
             * Targets are plugins that know how to generate code for
             * a specific language. Each target has one or more clients.
             */
            const target = targets.targets[targetId as keyof typeof targets.targets];
            if (!target) {
                console.warn(`Target "${targetId}" not found`);
                return false;
            }

            /**
             * Determine which client to use
             * 
             * If the user didn't specify a client, use the target's default.
             * For example, Python's default client is 'requests'.
             */
            const actualClientId = clientId || target.info.default;

            /**
             * Get the client converter function
             * 
             * Each client knows how to generate code for its specific library.
             * For example, the 'requests' client generates Python code using requests.
             */
            const client = target.clientsById[actualClientId as any];
            if (!client) {
                console.warn(`Client "${actualClientId}" not found for target "${targetId}"`);
                return false;
            }

            /**
             * Convert all stored requests to code
             * 
             * For each request, the client's convert function generates code.
             * The convert function receives:
             * - The processed request with parsed headers, query params, etc.
             * - Any options specified by the user
             */
            const results = this.requests.map((request) =>
                client.convert(request, options)
            );

            /**
             * Return single string or array
             * 
             * If there's only one request, return just the string.
             * If there are multiple requests, return an array of strings.
             * This makes the API easier to use for the common single-request case.
             */
            return results.length === 1 ? results[0] : results;
        } catch (err) {
            console.error('Error converting snippet:', err);
            return false;
        }
    }
}

/**
 * Re-export target information
 * 
 * This allows consumers of the library to discover:
 * - What programming languages are supported
 * - What HTTP clients are available for each language
 * - Default clients for each language
 * 
 * @example
 * ```typescript
 * import { targets } from './httpsnippet';
 * 
 * // List all supported languages
 * Object.keys(targets.targets).forEach(language => {
 *   const target = targets.targets[language];
 *   console.log(`${language}: ${target.info.title}`);
 * });
 * ```
 */
export { targets };
export type { TargetId } from './targets/targets';
export type { ClientId } from './targets/targets';
