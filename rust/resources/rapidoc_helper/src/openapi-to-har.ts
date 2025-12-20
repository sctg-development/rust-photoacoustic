// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

/**
 * OpenAPI to HTTP Archive (HAR) Converter
 * 
 * This module translates OpenAPI/Swagger API specifications into HTTP Archive (HAR) format.
 * HAR is a standardized JSON format for recording HTTP requests and responses.
 * 
 * What does this do?
 * - Reads OpenAPI 3.0 or Swagger 2.0 specifications
 * - Extracts all API endpoints with their request details
 * - Generates example HTTP requests for each endpoint
 * - Outputs requests in HAR format (HTTP Archive 1.2)
 * 
 * Why HAR format?
 * HAR is a widely supported standard that allows us to:
 * - Generate code snippets in multiple programming languages
 * - Create example API calls
 * - Record and replay HTTP transactions
 * - Share request/response information between tools
 * 
 * References:
 * - OpenAPI Specification: https://spec.openapis.org/
 * - Swagger 2.0 Specification: https://swagger.io/specification/
 * - HAR 1.2 Specification: http://www.softwareishard.com/blog/har-12-spec/
 * 
 * @module openapi-to-har
 * @version 1.0.0
 */
import { sample as _sample } from 'openapi-sampler';

// ============================================================================
// TYPE DEFINITIONS
// ============================================================================

/**
 * HTTP Parameter Object (HAR Format)
 * 
 * Represents a single parameter in an HTTP request, such as:
 * - Query parameters (e.g., ?page=1&limit=10)
 * - Headers (e.g., Authorization: Bearer token)
 * - Cookies (e.g., session=abc123)
 * - Form fields (e.g., username=john&password=secret)
 * 
 * @typedef {Object} HarParameterObject
 * @property {string} name - The parameter name/key
 * @property {string} value - The parameter value
 * 
 * @example
 * {name: 'page', value: '1'}
 * {name: 'Authorization', value: 'Bearer eyJhbGc...'}
 */
interface HarParameterObject {
    name: string;
    value: string;
}

/**
 * HTTP Request Body Data (HAR Format)
 * 
 * Represents the body/payload of an HTTP request.
 * Can be either:
 * - Raw text (for JSON, XML, plain text)
 * - Structured parameters (for form submissions)
 * 
 * @typedef {Object} HarPostData
 * @property {string} mimeType - Content-Type of the body (e.g., 'application/json')
 * @property {string} [text] - Raw text body content
 * @property {HarParameterObject[]} [params] - Form parameters array
 * 
 * @example
 * // JSON body
 * {mimeType: 'application/json', text: '{"name":"John"}'}
 * 
 * // Form data
 * {mimeType: 'application/x-www-form-urlencoded', params: [{name: 'username', value: 'john'}]}
 */
interface HarPostData {
    mimeType: string;
    text?: string;
    params?: HarParameterObject[];
}

/**
 * HTTP Request Object (HAR Format)
 * 
 * Represents a complete HTTP request in HAR format.
 * This is the core data structure used throughout this module.
 * 
 * @typedef {Object} HarRequestObject
 * @property {string} method - HTTP method (GET, POST, PUT, DELETE, etc.)
 * @property {string} url - Complete request URL
 * @property {HarParameterObject[]} headers - HTTP headers array
 * @property {HarParameterObject[]} queryString - Query string parameters
 * @property {string} httpVersion - HTTP version (e.g., 'HTTP/1.1')
 * @property {HarParameterObject[]} cookies - HTTP cookies array
 * @property {number} headersSize - Total headers size in bytes (for statistics)
 * @property {number} bodySize - Total body size in bytes (for statistics)
 * @property {HarPostData} [postData] - Request body
 * @property {string} [comment] - Optional description
 */
interface HarRequestObject {
    method: string;
    url: string;
    headers: HarParameterObject[];
    queryString: HarParameterObject[];
    httpVersion: string;
    cookies: HarParameterObject[];
    headersSize: number;
    bodySize: number;
    postData?: HarPostData;
    comment?: string;
}

/**
 * API Endpoint with Example Requests
 * 
 * Represents a single API endpoint extracted from an OpenAPI specification.
 * Contains one or more example HAR requests (for different request bodies, etc.)
 * 
 * @typedef {Object} HarEndpoint
 * @property {string} method - HTTP method (GET, POST, etc.)
 * @property {string} url - Endpoint URL path (e.g., '/users/{id}')
 * @property {string} description - Endpoint description from OpenAPI spec
 * @property {HarRequestObject[]} hars - Array of example requests for this endpoint
 * 
 * @example
 * {
 *   method: 'POST',
 *   url: 'https://api.example.com/users',
 *   description: 'Create a new user',
 *   hars: [{...request1...}, {...request2...}]
 * }
 */
interface HarEndpoint {
    method: string;
    url: string;
    description: string;
    hars: HarRequestObject[];
}

/**
 * OpenAPI Parameter Object
 * 
 * Describes a parameter that can appear in different locations:
 * - path: Part of the URL (e.g., /users/{id})
 * - query: Query string (e.g., ?page=1)
 * - header: HTTP header
 * - cookie: HTTP cookie
 * 
 * @typedef {Object} OpenAPIParameter
 * @property {string} name - Parameter name
 * @property {string} in - Where the parameter appears (path, query, header, cookie)
 * @property {string} [style] - How array values are serialized (form, simple, label, matrix, etc.)
 * @property {boolean} [explode] - Whether array elements are separated
 * @property {boolean} [required] - Whether this parameter is required
 * @property {string} [type] - Data type (string, integer, boolean, etc.)
 * @property {string} [format] - Data format (date, date-time, email, uuid, etc.)
 * @property {Object} [schema] - JSON Schema for the parameter
 * @property {unknown} [example] - Example value
 * @property {Record<string, unknown>} [examples] - Multiple example values
 * @property {unknown} [default] - Default value
 */
interface OpenAPIParameter {
    name: string;
    in: string;
    style?: string;
    explode?: boolean;
    required?: boolean;
    type?: string;
    format?: string;
    schema?: {
        type?: string;
        $ref?: string;
        example?: unknown;
        format?: string;
        minimum?: number;
        [key: string]: unknown;
    };
    example?: unknown;
    examples?: Record<string, unknown>;
    default?: unknown;
    $ref?: string;
    [key: string]: unknown;
}

/**
 * OpenAPI Server Information
 * 
 * Defines the base URL for API requests.
 * Can vary by environment (production, staging, development).
 * 
 * @typedef {Object} OpenAPIServer
 * @property {string} url - Base URL (e.g., 'https://api.example.com' or 'https://{host}:{port}/v1')
 * 
 * @example
 * {url: 'https://api.example.com/v1'}
 * {url: 'https://{environment}.example.com', variables: {...}}
 */
interface OpenAPIServer {
    url: string;
}

/**
 * OpenAPI Request Body
 * 
 * Describes what can be sent in the request body.
 * Typically contains multiple content types (JSON, form-data, etc.)
 * 
 * @typedef {Object} OpenAPIRequestBody
 * @property {Record<string, {schema: unknown}>} [content] - Content types and their schemas
 * @property {string} [$ref] - Reference to a schema definition
 * 
 * @example
 * {
 *   content: {
 *     'application/json': {schema: {...}},
 *     'application/x-www-form-urlencoded': {schema: {...}}
 *   }
 * }
 */
interface OpenAPIRequestBody {
    content?: Record<string, { schema: unknown }>;
    $ref?: string;
}

/**
 * OpenAPI Operation (HTTP Method)
 * 
 * Describes a single HTTP method on an endpoint.
 * An endpoint may have GET, POST, PUT, DELETE, etc. operations.
 * 
 * @typedef {Object} OpenAPIOperation
 * @property {OpenAPIParameter[]} [parameters] - Path, query, header, cookie parameters
 * @property {OpenAPIRequestBody} [requestBody] - Request body description
 * @property {Record<string, unknown>[]} [security] - Security requirements
 * @property {OpenAPIServer[]} [servers] - Override servers for this operation
 * @property {string} [description] - Human-readable description
 * @property {string[]} [consumes] - Accepted content types (Swagger 2.0)
 */
interface OpenAPIOperation {
    parameters?: OpenAPIParameter[];
    requestBody?: OpenAPIRequestBody;
    security?: Record<string, unknown>[];
    servers?: OpenAPIServer[];
    description?: string;
    consumes?: string[];
}

/**
 * OpenAPI Path Item
 * 
 * Describes all operations available at a single path.
 * Each path can have GET, POST, PUT, DELETE, etc. operations.
 * 
 * @typedef {Object} OpenAPIPath
 * @property {OpenAPIParameter[]} [parameters] - Parameters inherited by all operations
 * @property {OpenAPIServer[]} [servers] - Servers for all operations on this path
 * @property {OpenAPIOperation} [get] - GET operation
 * @property {OpenAPIOperation} [post] - POST operation
 * @property {OpenAPIOperation} [put] - PUT operation
 * @property {OpenAPIOperation} [delete] - DELETE operation
 * @property {OpenAPIOperation} [patch] - PATCH operation
 * @property {OpenAPIOperation} [head] - HEAD operation
 * @property {OpenAPIOperation} [options] - OPTIONS operation
 * @property {OpenAPIOperation} [trace] - TRACE operation
 */
interface OpenAPIPath {
    parameters?: OpenAPIParameter[];
    servers?: OpenAPIServer[];
    get?: OpenAPIOperation;
    post?: OpenAPIOperation;
    put?: OpenAPIOperation;
    delete?: OpenAPIOperation;
    patch?: OpenAPIOperation;
    head?: OpenAPIOperation;
    options?: OpenAPIOperation;
    trace?: OpenAPIOperation;
    [method: string]: OpenAPIOperation | OpenAPIParameter[] | OpenAPIServer[] | undefined;
}

/**
 * OpenAPI Security Scheme
 * 
 * Describes how to authenticate with the API.
 * Supports various security methods:
 * - apiKey: In header, query, or cookie
 * - http: HTTP Basic, Bearer token, etc.
 * - oauth2: OAuth 2.0 flows
 * - openIdConnect: OpenID Connect
 * 
 * @typedef {Object} OpenAPISecurityScheme
 * @property {string} type - Security scheme type
 * @property {string} [scheme] - HTTP authentication scheme
 * @property {string} [in] - Where to send the API key
 * @property {string} [name] - Name of the API key
 */
interface OpenAPISecurityScheme {
    type: string;
    scheme?: string;
    in?: string;
    name?: string;
}

/**
 * OpenAPI/Swagger Document
 * 
 * The root object of an OpenAPI/Swagger specification.
 * Contains all the information about the API: endpoints, schemas, security, etc.
 * 
 * @typedef {Object} OpenAPIDocument
 * @property {Record<string, OpenAPIPath>} paths - All API endpoints mapped by path
 * @property {OpenAPIServer[]} [servers] - API server URLs
 * @property {string[]} [schemes] - Supported schemes (http, https) - Swagger 2.0
 * @property {string} [host] - API host (e.g., 'api.example.com') - Swagger 2.0
 * @property {string} [basePath] - Base path prefix (e.g., '/v1') - Swagger 2.0
 * @property {Record<string, OpenAPISecurityScheme>} [securityDefinitions] - Security schemes - Swagger 2.0
 * @property {Object} [components] - Reusable components - OpenAPI 3.0
 * @property {Record<string, OpenAPISecurityScheme>} [components.securitySchemes] - Security schemes - OpenAPI 3.0
 * @property {Record<string, unknown>[]} [security] - Default security requirements
 */
interface OpenAPIDocument {
    paths: Record<string, OpenAPIPath>;
    servers?: OpenAPIServer[];
    schemes?: string[];
    host?: string;
    basePath?: string;
    securityDefinitions?: Record<string, OpenAPISecurityScheme>;
    components?: {
        securitySchemes?: Record<string, OpenAPISecurityScheme>;
    };
    security?: Record<string, unknown>[];
}

/**
 * Generate HAR Request from OpenAPI Endpoint
 * 
 * This function extracts information from an OpenAPI specification and creates
 * example HTTP requests in HAR format. If an endpoint has multiple request
 * examples (different content types, etc.), multiple HAR requests are created.
 * 
 * Steps:
 * 1. Build the base URL from servers/host configuration
 * 2. Get the full path with parameters resolved
 * 3. Extract headers, query parameters, and cookies
 * 4. Generate request payloads (JSON, form data, etc.)
 * 5. Create a HAR request for each payload variant
 * 
 * @param {OpenAPIDocument} openApi - The OpenAPI specification
 * @param {string} path - The endpoint path (e.g., '/users/{id}')
 * @param {string} method - HTTP method (get, post, put, delete, patch, head, options, trace)
 * @param {Record<string, unknown>} [queryParamValues={}] - Custom values for query parameters
 * @returns {HarRequestObject[]} Array of HAR request objects (one per payload variant)
 * 
 * @example
 * const hars = createHar(openApiDoc, '/users/{id}', 'get');
 * // Returns array of HAR requests, one for each example
 */
const createHar = function (
    openApi: OpenAPIDocument,
    path: string,
    method: string,
    queryParamValues: Record<string, unknown> = {}
): HarRequestObject[] {

    const baseUrl = getBaseUrl(openApi, path, method);

    /**
     * Create base HAR object with common properties
     * 
     * All request variants will share these properties.
     * Different variants (different payloads) will add postData field.
     */
    const baseHar = {
        method: method.toUpperCase(),
        url: baseUrl + getFullPath(openApi, path, method),
        headers: getHeadersArray(openApi, path, method),
        queryString: getQueryStrings(openApi, path, method, queryParamValues),
        httpVersion: 'HTTP/1.1',
        cookies: getCookies(openApi, path, method),
        headersSize: 0,
        bodySize: 0,
    };

    let hars: HarRequestObject[] = [];

    /**
     * Get all possible request payloads
     * 
     * An endpoint might accept multiple content types (JSON, form-data, etc.)
     * We create a HAR request for each content type variant.
     */
    const postDatas = getPayloads(openApi, path, method);

    /**
     * Create a HAR request for each payload variant
     * 
     * For each content type/payload combination:
     * 1. Deep copy the base HAR
     * 2. Add the payload
     * 3. Set the Content-Type header
     * 4. Add to results array
     */
    if (postDatas.length > 0) {
        for (let i in postDatas) {
            const postData = postDatas[i];
            // Deep copy base to avoid mutations
            const copiedHar = JSON.parse(JSON.stringify(baseHar));
            copiedHar.postData = postData;
            // Content-Type is stored in comment for reference
            copiedHar.comment = postData.mimeType;
            // Add Content-Type header
            copiedHar.headers.push({
                name: 'content-type',
                value: postData.mimeType,
            });
            hars.push(copiedHar);
        }
    } else {
        // No payload (GET, DELETE, HEAD, etc.)
        hars = [baseHar];
    }

    return hars;
};

/**
 * Check if a value is a JavaScript primitive type
 * 
 * Primitive types are the basic data types that are not objects:
 * - string: "hello"
 * - number: 42
 * - boolean: true/false
 * - null: null
 * 
 * Objects are complex types like arrays, objects, functions.
 * This check is used when deciding how to format parameters.
 * 
 * @param {unknown} value - The value to check
 * @returns {boolean} True if value is a primitive, false otherwise
 * 
 * @example
 * isPrimitive("hello")        // true
 * isPrimitive(42)             // true
 * isPrimitive({name: "John"}) // false
 * isPrimitive([1, 2, 3])      // false
 */
const isPrimitive = function (value: unknown): value is string | number | boolean | null {
    if (value === null) return true;
    const valueType = typeof value;
    if (valueType === 'function' || valueType === 'object') return false;
    return true;
};

/**
 * Get the prefix character for styled path parameters
 * 
 * OpenAPI supports different "styles" for serializing parameters.
 * Each style uses different prefix and separator characters.
 * 
 * Examples:
 * - simple style (default): /users/1,2,3
 * - label style: /users/.1.2.3 (prefix: '.')
 * - matrix style: /users;id=1;id=2 (prefix: ';')
 * 
 * This function returns the prefix for a given style.
 * 
 * @param {string} style - Parameter style (simple, label, matrix, form, etc.)
 * @returns {string} Prefix character (empty string for simple/form style)
 * 
 * @example
 * getPrefix('label')   // '.'
 * getPrefix('matrix')  // ';'
 * getPrefix('simple')  // ''
 */
const getPrefix = function (style: string): string {
    if (style === 'label') {
        return '.';
    }
    if (style === 'matrix') {
        return `;`;
    }
    return '';
};

/**
 * Get the separator character for styled array parameters
 * 
 * When a parameter contains multiple values (array), different styles
 * use different separators to combine them.
 * 
 * Examples:
 * - simple/form: 1,2,3 (comma separator)
 * - label: .1.2.3 (dot separator)
 * - matrix: ;id=1;id=2 (semicolon separator)
 * 
 * @param {string} style - Parameter style
 * @returns {string} Separator character
 * 
 * @example
 * getSeparator('label')   // '.'
 * getSeparator('matrix')  // ';'
 * getSeparator('simple')  // ','
 */
const getSeparator = function (style: string): string {
    if (style === 'label') return '.';
    if (style === 'matrix') return ';';
    return ',';
};

/**
 * Get parameter identifier for matrix style parameters
 * 
 * Matrix style parameters need an identifier after the separator:
 * Example: /users;id=1;id=2
 * 
 * The identifier "id=" is needed to know which parameter each value belongs to.
 * Other styles don't need this because values are positional.
 * 
 * @param {string} style - Parameter style
 * @param {string} name - Parameter name
 * @returns {string} Identifier string (e.g., "id=" or empty string)
 * 
 * @example
 * getParamId('matrix', 'id')     // 'id='
 * getParamId('label', 'id')      // ''
 */
const getParamId = function (style: string, name: string): string {
    if (style === 'matrix') return `${name}=`;
    return '';
};

/**
 * Get the default serialization style for a parameter location
 * 
 * According to OpenAPI 3.0.3 specification, different parameter locations
 * have recommended default styles for how to serialize parameter values.
 * 
 * - Path and Header parameters default to 'simple' style
 *   Example: simple style uses comma separator: id=1,2,3
 * 
 * - Query and Cookie parameters default to 'form' style
 *   Example: form style expands repeated params: id=1&id=2&id=3
 * 
 * The style affects how arrays and objects are formatted in the request.
 * 
 * @param {string} location - Parameter location: 'path', 'query', 'header', or 'cookie'
 * @returns {string} Default style for the location (e.g., 'simple' or 'form')
 * 
 * @example
 * getDefaultStyleForLocation('path')      // 'simple'
 * getDefaultStyleForLocation('query')     // 'form'
 * getDefaultStyleForLocation('header')    // 'simple'
 * getDefaultStyleForLocation('cookie')    // 'form'
 * 
 * @see {@link https://spec.openapis.org/oas/v3.0.3#parameter-object} OpenAPI 3.0.3 Parameter Object Spec
 */
const getDefaultStyleForLocation = function (location: string): string {
    if (location === 'path' || location === 'header') {
        return 'simple';
    } else if (location === 'query' || location === 'cookie') {
        return 'form';
    }
    return '';
};

/**
 * Get the default "explode" setting for a given parameter serialization style
 * 
 * The "explode" property controls whether array and object parameters are
 * expanded into multiple name-value pairs or kept as a single pair.
 * 
 * Explode=true (expanded):
 *   form style: id=1&id=2&id=3 (multiple parameters with same name)
 *   query style: name=Alice&name=Bob (multiple parameters)
 * 
 * Explode=false (not expanded/compact):
 *   form style: id=1,2,3 (single parameter with comma-separated values)
 *   query style: id=1,2,3 (single parameter with comma-separated values)
 * 
 * The 'form' style is the only one that defaults to explode=true.
 * All other styles (simple, label, matrix) default to explode=false.
 * 
 * @param {string} style - Parameter style: 'simple', 'form', 'label', 'matrix', etc.
 * @returns {boolean} True if the style should be exploded by default, false otherwise
 * 
 * @example
 * getDefaultExplodeForStyle('form')      // true
 * getDefaultExplodeForStyle('simple')    // false
 * getDefaultExplodeForStyle('label')     // false
 * getDefaultExplodeForStyle('matrix')    // false
 * 
 * @see {@link https://spec.openapis.org/oas/v3.0.3#fixed-fields-9} OpenAPI 3.0.3 Parameter Explode
 */
const getDefaultExplodeForStyle = function (style: string): boolean {
    return style === 'form';
};

/**
 * Get the separator for array elements in unexploded array parameters
 * 
 * When serializing an array parameter with explode=false (compact form),
 * the array elements need to be joined with a separator character.
 * 
 * Different styles use different separators:
 * - Standard separator: comma (,)
 *   Example: ids=1,2,3
 * 
 * - Space-delimited: space ( )
 *   Example: ids=1 2 3
 *   Used when style='spaceDelimited'
 * 
 * - Pipe-delimited: pipe (|)
 *   Example: ids=1|2|3
 *   Used when style='pipeDelimited'
 * 
 * These delimited styles are useful for query parameters to provide
 * alternative serialization when comma might be ambiguous.
 * 
 * @param {string} style - Parameter style name
 * @returns {string} Separator character to use between array elements
 * 
 * @example
 * getArrayElementSeparator('form')              // ','
 * getArrayElementSeparator('spaceDelimited')    // ' '
 * getArrayElementSeparator('pipeDelimited')     // '|'
 */
const getArrayElementSeparator = function (style: string): string {
    let separator = ',';
    if (style === 'spaceDelimited') {
        separator = ' ';
    } else if (style === 'pipeDelimited') {
        separator = '|';
    }
    return separator;
};

/**
 * Join object key-value pairs into a single string
 * 
 * This function converts a JavaScript object into a string representation
 * by joining keys and values with specified separators.
 * 
 * The separators control:
 * - keyValueSeparator: what goes between a key and its value
 * - pairSeparator: what goes between different key-value pairs
 * 
 * Example transformations:
 * { firstName: 'Alex', age: 34 } with ('=', ',')
 * → 'firstName=Alex,age=34'
 * 
 * This is used when serializing object parameters in different OpenAPI styles.
 * For example, in matrix style with explode=false:
 * { id: 1, name: 'test' } → id=1,name=test (with ';' pair separator)
 * 
 * @param {Record<string, unknown>} obj - Object to convert to string
 * @param {string} [keyValueSeparator='='] - String between key and value
 * @param {string} [pairSeparator=','] - String between key-value pairs
 * @returns {string} String representation of the object
 * 
 * @example
 * // Returns "firstName=Alex,age=34"
 * objectJoin({ firstName: 'Alex', age: 34 }, '=', ',')
 * 
 * // Returns "firstName:Alex;age:34"
 * objectJoin({ firstName: 'Alex', age: 34 }, ':', ';')
 * 
 * // Returns "firstName Alex age 34" (space-separated)
 * objectJoin({ firstName: 'Alex', age: 34 }, ' ', ' ')
 */
const objectJoin = function (
    obj: Record<string, unknown>,
    keyValueSeparator: string = ',',
    pairSeparator: string = ','
): string {
    return Object.entries(obj)
        .map(([k, v]) => `${k}${keyValueSeparator}${v}`)
        .join(pairSeparator);
};

/**
 * Convert an OpenAPI parameter into HAR parameter objects
 * 
 * This function takes a single OpenAPI parameter definition with a value
 * and converts it to one or more HAR parameter objects that can be used
 * in an HTTP request.
 * 
 * The conversion handles different OpenAPI parameter styles and serialization:
 * 
 * SIMPLE STYLE (default for path/header):
 * - Primitive values: "id=5"
 * - Arrays: "id=5,6,7" or "id=5;id=6;id=7" (if explode=true)
 * - Objects: "firstName=John,age=30" or with commas/semicolons
 * 
 * FORM STYLE (default for query/cookie):
 * - Primitive values: single parameter "id=5"
 * - Arrays with explode=true: multiple params "id=5&id=6&id=7"
 * - Arrays with explode=false: "id=5,6,7"
 * - Objects with explode=true: multiple params "firstName=John&age=30"
 * - Objects with explode=false: "firstName,John,age,30"
 * 
 * DEEPOBJECT STYLE (for query only):
 * - Only for objects: "filter[name]=John&filter[age]=30"
 * 
 * MATRIX STYLE (for path parameters):
 * - Primitive: ";id=5"
 * - Arrays: ";id=5;id=6"
 * - Objects: ";firstName=John;age=30"
 * 
 * LABEL STYLE (for path parameters):
 * - Primitive: ".5"
 * - Arrays: ".5.6.7" or ".5.6.7" (with explode)
 * - Objects: ".firstName=John.age=30"
 * 
 * @param {Object} paramDef - OpenAPI parameter definition
 * @param {string} paramDef.name - Parameter name (required)
 * @param {string} paramDef.in - Parameter location: 'path', 'query', 'header', 'cookie' (required)
 * @param {string} [paramDef.style] - Serialization style (overrides default)
 * @param {boolean} [paramDef.explode] - Whether to expand arrays/objects (overrides default)
 * @param {unknown} value - The value to convert (string, number, boolean, array, or object)
 * 
 * @returns {HarParameterObject[]} Array of HAR parameters
 * 
 * @throws {string} If required parameters (name, in, value) are missing
 * 
 * @example
 * // Simple primitive value
 * createHarParameterObjects({name: 'id', in: 'query'}, 5)
 * // → [{name: 'id', value: '5'}]
 * 
 * // Array with default form style (exploded)
 * createHarParameterObjects({name: 'id', in: 'query'}, [1, 2, 3])
 * // → [{name: 'id', value: '1'}, {name: 'id', value: '2'}, {name: 'id', value: '3'}]
 * 
 * // Array with form style (not exploded)
 * createHarParameterObjects({name: 'id', in: 'query', explode: false}, [1, 2, 3])
 * // → [{name: 'id', value: '1,2,3'}]
 * 
 * // Object with deepObject style (query only)
 * createHarParameterObjects({name: 'filter', in: 'query', style: 'deepObject'}, {name: 'John', age: 30})
 * // → [{name: 'filter[name]', value: 'John'}, {name: 'filter[age]', value: '30'}]
 */
export const createHarParameterObjects = function (
    { name, in: location, style, explode }: OpenAPIParameter,
    value: unknown
): HarParameterObject[] {
    if (!name || !location || typeof value === 'undefined') {
        throw 'Required parameters missing';
    }

    const prefix = getPrefix(style || '');
    const paramId = getParamId(style || '', name);

    if (isPrimitive(value)) {
        return [{ name, value: prefix + paramId + value }];
    }

    const objects: HarParameterObject[] = [];
    const actualStyle = style || getDefaultStyleForLocation(location);
    const actualExplode = explode ?? getDefaultExplodeForStyle(actualStyle);

    if (location === 'query' || location === 'cookie') {
        const separator = getArrayElementSeparator(actualStyle);
        if (Array.isArray(value)) {
            if (actualExplode) {
                objects.push(
                    ...value.map((entry) => {
                        return { name, value: String(entry) };
                    })
                );
            } else {
                objects.push({ name, value: value.map(String).join(separator) });
            }
        } else if (value && typeof value === 'object') {
            if (actualStyle === 'deepObject') {
                objects.push(
                    ...Object.entries(value as Record<string, unknown>).map(([k, v]) => {
                        return { name: `${name}[${k}]`, value: String(v) };
                    })
                );
            } else if (actualExplode) {
                objects.push(
                    ...Object.entries(value as Record<string, unknown>).map(([k, v]) => {
                        return { name: k, value: String(v) };
                    })
                );
            } else {
                objects.push({
                    name,
                    value: objectJoin(value as Record<string, unknown>),
                });
            }
        }
    } else if (location === 'path' || location === 'header') {
        const separator = getSeparator(actualStyle);

        if (Array.isArray(value)) {
            objects.push({
                name,
                value:
                    prefix + paramId + value.map(String).join(actualExplode ? separator + paramId : ','),
            });
        } else if (value && typeof value === 'object') {
            if (actualExplode) {
                objects.push({
                    name,
                    value: prefix + objectJoin(value as Record<string, unknown>, '=', separator),
                });
            } else {
                objects.push({
                    name,
                    value: prefix + paramId + objectJoin(value as Record<string, unknown>),
                });
            }
        }
    }

    return objects;
};

/**
 * Format sample data from OpenAPI examples into HAR parameter objects
 * 
 * This function converts sample data (typically from OpenAPI examples or generated values)
 * into HAR parameter format that can be used in HTTP requests.
 * 
 * It handles nested structures:
 * - Simple values: {name: "John"} → [{name: 'name', value: 'John'}]
 * - Arrays: {ids: [1,2,3]} → [{name: 'ids[]', value: '1'}, {name: 'ids[]', value: '2'}, ...]
 * - Nested objects: {user: {name: "John", age: 30}} → [{name: 'user[name]', value: 'John'}, ...]
 * - Nested arrays: {user: {tags: ["admin", "user"]}} → [{name: 'user[tags][]', value: 'admin'}, ...]
 * 
 * This is commonly used for form data and application/x-www-form-urlencoded body content,
 * where complex nested data must be flattened into a series of key-value pairs.
 * 
 * @param {Record<string, unknown>} sample - Object containing sample data to format
 * @returns {HarParameterObject[]} Array of HAR parameters (flat key-value pairs)
 * 
 * @example
 * // Simple values
 * formatSamples({name: 'John', age: 30})
 * // → [{name: 'name', value: 'John'}, {name: 'age', value: '30'}]
 * 
 * // Arrays
 * formatSamples({ids: [1, 2, 3]})
 * // → [{name: 'ids[]', value: '1'}, {name: 'ids[]', value: '2'}, {name: 'ids[]', value: '3'}]
 * 
 * // Nested objects
 * formatSamples({user: {name: 'John', email: 'john@example.com'}})
 * // → [{name: 'user[name]', value: 'John'}, {name: 'user[email]', value: 'john@example.com'}]
 * 
 * // Nested arrays
 * formatSamples({user: {tags: ['admin', 'user']}})
 * // → [{name: 'user[tags][]', value: 'admin'}, {name: 'user[tags][]', value: 'user'}]
 */
const formatSamples = function (sample: Record<string, unknown>): HarParameterObject[] {
    const params: HarParameterObject[] = [];

    Object.keys(sample).map((key) => {
        // console.log(`key=${JSON.stringify(key, null, 4)} (${typeof(sample[key])})`);
        if (Array.isArray(sample[key])) {
            // console.log("Array.isArray(sample[key])");
            (sample[key] as unknown[]).forEach((entry) => {
                // console.log(`entry=${JSON.stringify(entry, null, 4)}\n`);
                params.push({
                    name: `${key}[]`,
                    value: String(entry),
                });
            });
        } else if (Object.prototype.toString.call(sample[key]) === '[object Object]') {
            // console.log("Object.prototype.toString.call(sample[key]) === '[object Object]'");
            const obj = sample[key] as Record<string, unknown>;
            Object.keys(obj).map((k) => {
                // console.log(`k=${JSON.stringify(k, null, 4)}\n`);
                if (Array.isArray(obj[k])) {
                    // console.log("Array.isArray(sample[key][k])");
                    (obj[k] as unknown[]).forEach((entry) => {
                        // console.log(`entry=${JSON.stringify(entry, null, 4)}\n`);
                        params.push({
                            name: `${key}[${k}][]`,
                            value: String(entry),
                        });
                    });
                } else {
                    params.push({
                        name: key + '[' + k + ']',
                        value: String(obj[k]),
                    });
                }
            });
        } else {
            // console.log("else\n");
            params.push({
                name: key,
                value: String(sample[key]),
            });
        }
    });

    return params;
}

/**
 * Generate request body payloads for a given OpenAPI endpoint
 * 
 * This function extracts the request body definition from an OpenAPI operation
 * and generates example payloads for each supported content type (JSON, form-data, etc).
 * 
 * It supports multiple content types and generates an example for each:
 * 
 * 1. application/json
 *    - Uses OpenAPI schema to generate a sample JSON object
 *    - Returns payload with JSON string representation
 *    - Example: {mimeType: 'application/json', text: '{"name":"John","age":30}'}
 * 
 * 2. multipart/form-data
 *    - Converts object properties to form fields
 *    - Each property becomes a form parameter
 *    - Example: {mimeType: 'multipart/form-data', params: [{name: 'username', value: 'John'}]}
 * 
 * 3. application/x-www-form-urlencoded
 *    - Flattens nested objects into form parameters
 *    - Handles arrays with [] notation (key[]=value1&key[]=value2)
 *    - Includes both params array and text representation
 *    - Example: {mimeType: 'application/x-www-form-urlencoded', params: [...], text: 'username=John&age=30'}
 * 
 * The function also handles Swagger 2.0 "body" parameter format (deprecated in OpenAPI 3.0).
 * 
 * Algorithm:
 * 1. Check for Swagger 2.0 body parameter with in='body'
 * 2. If not found, look for OpenAPI 3.0 requestBody.content
 * 3. For each content type (json, form-data, url-encoded):
 *    - Generate example values from schema using openapi-sampler
 *    - Format according to content type
 *    - Create HarPostData object
 * 4. Return array of payloads (one per content type)
 * 
 * @param {OpenAPIDocument} openApi - Complete OpenAPI specification
 * @param {string} path - API endpoint path (e.g., '/users')
 * @param {string} method - HTTP method (e.g., 'post', 'put')
 * @returns {HarPostData[]} Array of request payloads, one for each content type
 * 
 * @example
 * // For endpoint with JSON body
 * getPayloads(openApi, '/users', 'post')
 * // → [{mimeType: 'application/json', text: '{"name":"John","email":"john@example.com"}'}]
 * 
 * // For endpoint with form-data
 * getPayloads(openApi, '/upload', 'post')
 * // → [{mimeType: 'multipart/form-data', params: [{name: 'file', value: '[File]'}, ...]}]
 * 
 * // For endpoint with URL-encoded form
 * getPayloads(openApi, '/login', 'post')
 * // → [{mimeType: 'application/x-www-form-urlencoded', params: [...], text: 'username=user&password=pass'}]
 */
const getPayloads = function (openApi: OpenAPIDocument, path: string, method: string): HarPostData[] {
    if (typeof (openApi.paths[path][method] as OpenAPIOperation).parameters !== 'undefined') {
        const params = (openApi.paths[path][method] as OpenAPIOperation).parameters!;
        for (let i in params) {
            const param = params[i];
            if (
                typeof param.in !== 'undefined' &&
                param.in.toLowerCase() === 'body' &&
                typeof param.schema !== 'undefined'
            ) {
                try {
                    const sample = _sample(
                        param.schema as any,
                        { skipReadOnly: true },
                        openApi
                    );
                    return [
                        {
                            mimeType: 'application/json',
                            text: JSON.stringify(sample),
                        },
                    ];
                } catch (err) {
                    console.log(err);
                    return [];
                }
            }
        }
    }

    const methodObj = openApi.paths[path][method] as OpenAPIOperation;
    if (methodObj.requestBody && methodObj.requestBody['$ref']) {
        methodObj.requestBody = resolveRef(openApi, methodObj.requestBody['$ref']) as OpenAPIRequestBody;
    }

    const payloads: HarPostData[] = [];
    if (methodObj.requestBody && methodObj.requestBody.content) {
        [
            'application/json',
            'application/x-www-form-urlencoded',
            'multipart/form-data',
        ].forEach((type) => {
            const content = methodObj.requestBody!.content![type];
            if (content && content.schema) {
                // console.log(JSON.stringify(content, null, 4) + "\n");
                const sample = _sample(
                    content.schema,
                    { skipReadOnly: true },
                    openApi
                );
                if (type === 'application/json') {
                    payloads.push({
                        mimeType: type,
                        text: JSON.stringify(sample),
                    });
                } else if (type === 'multipart/form-data') {
                    if (sample !== undefined && sample !== null && typeof sample === 'object') {
                        const params: HarParameterObject[] = [];
                        Object.keys(sample as Record<string, unknown>).forEach((key) => {
                            const sampleObj = sample as Record<string, unknown>;
                            let value = sampleObj[key];
                            if (typeof sampleObj[key] !== 'string') {
                                value = JSON.stringify(sampleObj[key]);
                            }
                            params.push({ name: key, value: String(value) });
                        });
                        payloads.push({
                            mimeType: type,
                            params: params,
                        });
                    }
                } else if (type == 'application/x-www-form-urlencoded') {
                    if (sample === undefined) return;

                    // console.log(`sample=${JSON.stringify(sample, null, 4)}`);
                    const params = formatSamples(sample as Record<string, unknown>);

                    payloads.push({
                        mimeType: 'application/x-www-form-urlencoded',
                        params: params,
                        text: params
                            .map((p) => p.name + '=' + p.value)
                            .join('&'),
                    });
                }
            }
        });
    }
    return payloads;
};

/**
 * Get the base URL (protocol + host + basePath) from OpenAPI specification
 * 
 * The base URL is constructed by looking for servers/hosts in this priority order:
 * 
 * 1. Operation-level servers (operation.servers)
 *    - Most specific, only for this HTTP method on this path
 *    - Example: GET /admin endpoints might have different servers than public ones
 * 
 * 2. Path-level servers (paths./{path}.servers)
 *    - Applies to all HTTP methods on this path
 *    - Example: all operations under /api paths
 * 
 * 3. Root-level servers (servers array)
 *    - Default for entire API specification
 *    - Example: main production/sandbox URLs
 * 
 * 4. Swagger 2.0 format (schemes, host, basePath)
 *    - Fallback for older API specifications
 *    - schemes: ['https', 'http']
 *    - host: 'api.example.com'
 *    - basePath: '/v1' (optional)
 * 
 * 5. Default fallback
 *    - If no configuration found: 'http://localhost:8080'
 *    - Used during development or incomplete specifications
 * 
 * @param {OpenAPIDocument} openApi - Complete OpenAPI/Swagger specification
 * @param {string} path - API endpoint path (used to find path-level servers)
 * @param {string} method - HTTP method (used to find operation-level servers)
 * @returns {string} Complete base URL including protocol and host
 * 
 * @example
 * // With OpenAPI 3.0 servers
 * getBaseUrl(openApi, '/users', 'get')
 * // → 'https://api.example.com'
 * 
 * // With Swagger 2.0 format
 * getBaseUrl(swagger2Doc, '/users', 'get')
 * // → 'https://api.example.com/v1'
 * 
 * // With fallback
 * getBaseUrl(minimalDoc, '/users', 'get')
 * // → 'http://localhost:8080'
 */
const getBaseUrl = function (openApi: OpenAPIDocument, path: string, method: string): string {
    const methodObj = openApi.paths[path][method] as OpenAPIOperation;
    if (methodObj.servers) return methodObj.servers[0].url;
    if (openApi.paths[path].servers) return openApi.paths[path].servers![0].url;
    if (openApi.servers) return openApi.servers[0].url;

    let baseUrl = '';
    if (typeof openApi.schemes !== 'undefined') {
        baseUrl += (openApi.schemes as string[])[0];
    } else {
        baseUrl += 'http';
    }

    if (openApi.host) {
        if (openApi.basePath === '/') {
            baseUrl += '://' + openApi.host;
        } else {
            baseUrl += '://' + openApi.host + (openApi.basePath || '/');
        }
    } else {
        // If no host is defined, return just the protocol
        baseUrl += '://localhost:8080';
    }

    return baseUrl;
};

/**
 * Get example values for a single OpenAPI parameter
 * 
 * This function retrieves the value to use for a parameter in an HTTP request.
 * It uses a priority-based strategy to find the best example value:
 * 
 * 1. Custom values (if provided)
 *    - Override values passed by the caller
 *    - Highest priority for testing with specific values
 * 
 * 2. Parameter example
 *    - Defined directly in parameter definition: parameter.example
 *    - Explicitly set by API designer
 * 
 * 3. Parameter examples (plural)
 *    - Examples object with named variations: parameter.examples
 *    - Uses the first available example
 *    - May be referenced via $ref
 * 
 * 4. Schema example
 *    - Defined in the schema: parameter.schema.example
 *    - For complex parameter types with schema
 * 
 * 5. Schema default
 *    - Default value in schema: parameter.default
 *    - Fallback if no examples defined
 * 
 * 6. Generated placeholder
 *    - Auto-generated value: "SOME_<TYPE>_VALUE"
 *    - For strings: "SOME_STRING_VALUE"
 *    - For numbers: "SOME_INTEGER_VALUE"
 *    - Special case for path parameters: keeps placeholder like "{id}"
 *    - Used when no examples available
 * 
 * Special handling for path parameters:
 * - Defaults to placeholder format: {paramName} (e.g., {id})
 * - This allows dynamic substitution later
 * - Prevents breaking the URL structure during generation
 * 
 * @param {OpenAPIDocument} openApi - Full OpenAPI specification (for resolving references)
 * @param {OpenAPIParameter} param - Parameter definition
 * @param {string} location - Parameter location: 'path', 'query', 'header', 'cookie'
 * @param {Record<string, unknown>} [values] - Custom override values for parameters
 * @returns {HarParameterObject[]} Array of HAR parameter objects with example values
 * 
 * @example
 * // Using parameter example
 * getParameterValues(openApi, {name: 'page', in: 'query', example: 2}, 'query')
 * // → [{name: 'page', value: '2'}]
 * 
 * // Using generated value
 * getParameterValues(openApi, {name: 'limit', in: 'query', type: 'integer'}, 'query')
 * // → [{name: 'limit', value: 'SOME_INTEGER_VALUE'}]
 * 
 * // Path parameter (keeps placeholder)
 * getParameterValues(openApi, {name: 'id', in: 'path'}, 'path')
 * // → [{name: 'id', value: '{id}'}]
 * 
 * // Custom override
 * getParameterValues(openApi, {name: 'id', in: 'path'}, 'path', {id: '123'})
 * // → [{name: 'id', value: '123'}]
 */
const getParameterValues = function (
    openApi: OpenAPIDocument,
    param: OpenAPIParameter,
    location: string,
    values?: Record<string, unknown>
): HarParameterObject[] {
    const paramType = param.type || (param.schema && typeof param.schema === 'object' && param.schema.type) || 'STRING';
    const typeStr = typeof paramType === 'string' ? paramType : 'STRING';
    let value: unknown = 'SOME_' + typeStr.toUpperCase() + '_VALUE';
    if (location === 'path') {
        // then default to the original place holder value (e.b. '{id}')
        value = `{${param.name}}`;
    }

    if (values && typeof values[param.name] !== 'undefined') {
        value = values[param.name];
    } else if (typeof param.example !== 'undefined') {
        value = param.example;
    } else if (typeof param.examples !== 'undefined') {
        let firstExample = Object.values(param.examples)[0] as any;
        if (
            typeof firstExample['$ref'] === 'string' &&
            /^#/.test(firstExample['$ref'])
        ) {
            firstExample = resolveRef(openApi, firstExample['$ref']);
        }
        value = (firstExample as any).value;
    } else if (
        typeof param.schema !== 'undefined' &&
        typeof param.schema.example !== 'undefined'
    ) {
        value = param.schema.example;
    } else if (typeof param.default !== 'undefined') {
        value = param.default;
    }

    return createHarParameterObjects(param, value);
};

/**
 * Parse OpenAPI parameters into HAR parameter objects grouped by name
 * 
 * This function extracts parameters from an OpenAPI specification that are located
 * in a specific position (query, path, header, cookie) and converts them to
 * HAR format with example values.
 * 
 * Process:
 * 1. Iterate through all parameters
 * 2. Resolve $ref references if needed
 * 3. Filter to only parameters in the specified location
 * 4. Handle schema $ref references
 * 5. Get example values using getParameterValues
 * 6. Group results by parameter name in an object
 * 
 * Grouping by name is useful because:
 * - Some parameters may expand into multiple HAR objects (arrays, objects)
 * - Need to track them by parameter name
 * - Later code flattens the groups back into a single array
 * 
 * @param {OpenAPIDocument} openApi - Full OpenAPI specification
 * @param {OpenAPIParameter[]} parameters - Array of parameter definitions to parse
 * @param {string} location - Filter parameters to only this location: 'path', 'query', 'header', 'cookie'
 * @param {Record<string, unknown>} [values] - Custom override values for specific parameters
 * @returns {Record<string, HarParameterObject[]>} Object mapping parameter names to arrays of HAR objects
 * 
 * @example
 * // Returns object like:
 * // {
 * //   "id": [{name: "id", value: "123"}],
 * //   "filter": [{name: "filter[name]", value: "test"}]
 * // }
 * parseParametersToQuery(openApi, params, 'query')
 */
const parseParametersToQuery = function (
    openApi: OpenAPIDocument,
    parameters: OpenAPIParameter[],
    location: string,
    values?: Record<string, unknown>
): Record<string, HarParameterObject[]> {
    const queryStrings: Record<string, HarParameterObject[]> = {};

    for (let i in parameters) {
        let param = parameters[i] as any;
        if (typeof param['$ref'] === 'string' && /^#/.test(param['$ref'])) {
            param = resolveRef(openApi, param['$ref']);
        }
        if (typeof param.schema !== 'undefined') {
            if (
                typeof param.schema['$ref'] === 'string' &&
                /^#/.test(param.schema['$ref'])
            ) {
                param.schema = resolveRef(openApi, param.schema['$ref']);
                if (typeof param.schema.type === 'undefined') {
                    // many schemas don't have an explicit type
                    param.schema.type = 'object';
                }
            }
        }
        if (
            typeof param.in !== 'undefined' &&
            param.in.toLowerCase() === location
        ) {
            // param.name is a safe key, because the spec defines
            // that name MUST be unique
            queryStrings[param.name] = getParameterValues(
                openApi,
                param,
                location,
                values
            );
        }
    }

    return queryStrings;
};

/**
 * Get all parameters of a specific type (location) for an endpoint
 * 
 * This function collects parameters from two levels of the OpenAPI specification:
 * 
 * Path-level parameters:
 * - Defined in paths./{path}.parameters
 * - Apply to ALL HTTP methods on that path
 * - Can be overridden at operation level
 * - Example: shared path parameters like {id} used by GET, PUT, DELETE
 * 
 * Operation-level parameters:
 * - Defined in paths./{path}.{method}.parameters
 * - Only apply to this specific HTTP method
 * - Override path-level parameters with same name
 * - Example: only POST uses certain parameters
 * 
 * The function merges parameters at both levels, with operation-level taking
 * precedence over path-level parameters with the same name.
 * 
 * @param {OpenAPIDocument} openApi - Full OpenAPI specification
 * @param {string} path - API endpoint path (e.g., '/users/{id}')
 * @param {string} method - HTTP method (e.g., 'get', 'post', 'put')
 * @param {string} location - Parameter location filter: 'path', 'query', 'header', 'cookie'
 * @param {Record<string, unknown>} [values] - Custom override values for parameters
 * @returns {HarParameterObject[]} Flat array of all parameters in that location
 * 
 * @example
 * // Get all query parameters for GET /users/{id}
 * getParameterCollectionIn(openApi, '/users/{id}', 'get', 'query')
 * // → [{name: 'filter', value: '...'}, {name: 'limit', value: '...'}]
 * 
 * // Get all path parameters (typically just {id})
 * getParameterCollectionIn(openApi, '/users/{id}', 'get', 'path')
 * // → [{name: 'id', value: '{id}'}]
 */
const getParameterCollectionIn = function (
    openApi: OpenAPIDocument,
    path: string,
    method: string,
    location: string,
    values?: Record<string, unknown>
): HarParameterObject[] {
    // Set the optional parameter if it's not provided
    if (typeof values === 'undefined') {
        values = {};
    }

    let pathParameters: Record<string, HarParameterObject[]> = {};
    let operationParameters: Record<string, HarParameterObject[]> = {};

    // First get any parameters from the path
    if (typeof openApi.paths[path].parameters !== 'undefined') {
        pathParameters = parseParametersToQuery(
            openApi,
            openApi.paths[path].parameters!,
            location,
            values
        );
    }

    const methodObj = openApi.paths[path][method] as OpenAPIOperation;
    if (typeof methodObj.parameters !== 'undefined') {
        operationParameters = parseParametersToQuery(
            openApi,
            methodObj.parameters!,
            location,
            values
        );
    }

    // Merge parameters, with method overriding path
    const queryStrings: Record<string, HarParameterObject[]> = Object.assign(pathParameters, operationParameters);

    // Convert the list of lists in Object.values(queryStrings) into a list
    const result: HarParameterObject[] = [];
    Object.values(queryStrings).forEach((entry) => {
        result.push(...entry);
    });
    return result;
};

/**
 * Get all query parameters for an OpenAPI endpoint
 * 
 * A convenience function that calls getParameterCollectionIn with location='query'.
 * Query parameters are those passed in the URL query string after the '?' character.
 * 
 * Example URL: GET /users?filter=active&limit=10&page=1
 * - filter: query parameter with value "active"
 * - limit: query parameter with value "10"
 * - page: query parameter with value "1"
 * 
 * Query parameters are typically optional and used for filtering, pagination,
 * sorting, and other non-critical request options.
 * 
 * @param {OpenAPIDocument} openApi - Full OpenAPI specification
 * @param {string} path - API endpoint path
 * @param {string} method - HTTP method
 * @param {Record<string, unknown>} [values] - Custom override values
 * @returns {HarParameterObject[]} Array of query parameters with example values
 * 
 * @example
 * getQueryStrings(openApi, '/users', 'get')
 * // → [{name: 'page', value: '1'}, {name: 'limit', value: '10'}]
 */
const getQueryStrings = function (
    openApi: OpenAPIDocument,
    path: string,
    method: string,
    values?: Record<string, unknown>
): HarParameterObject[] {
    return getParameterCollectionIn(openApi, path, method, 'query', values);
};

/**
 * Construct the full URL path with path parameters substituted
 * 
 * OpenAPI paths often contain path parameters in curly braces: /users/{id}
 * This function replaces those placeholders with example values.
 * 
 * Process:
 * 1. Get all path-location parameters (those in {curly braces})
 * 2. Extract their example values
 * 3. Replace {paramName} with the example value in the path
 * 4. Return the fully resolved path
 * 
 * Example:
 * - Original path: /users/{userId}/posts/{postId}
 * - Parameters: userId=123, postId=456
 * - Result: /users/123/posts/456
 * 
 * The full URL is constructed by: getBaseUrl() + getFullPath()
 * 
 * @param {OpenAPIDocument} openApi - Full OpenAPI specification
 * @param {string} path - OpenAPI path template (e.g., '/users/{id}')
 * @param {string} method - HTTP method
 * @returns {string} Full path with example values substituted
 * 
 * @example
 * getFullPath(openApi, '/users/{id}', 'get')
 * // → '/users/123'
 * 
 * getFullPath(openApi, '/users/{userId}/posts/{postId}', 'get')
 * // → '/users/42/posts/99'
 */
const getFullPath = function (
    openApi: OpenAPIDocument,
    path: string,
    method: string
): string {
    let fullPath = path;

    const pathParameters = getParameterCollectionIn(
        openApi,
        path,
        method,
        'path'
    );
    pathParameters.forEach(({ name, value }) => {
        fullPath = fullPath.replace('{' + name + '}', value);
    });

    return fullPath;
};

/**
 * Get all cookie parameters for an OpenAPI endpoint
 * 
 * A convenience function that calls getParameterCollectionIn with location='cookie'.
 * Cookies are sent in the HTTP Cookie header and are typically used for
 * session management, authentication, and client state tracking.
 * 
 * Example HTTP header: Cookie: sessionId=abc123; preferences=dark-mode
 * 
 * In OpenAPI, cookies are defined as parameters with in='cookie'.
 * Unlike query and path parameters, cookies are less commonly used for API
 * operations, but may be found in APIs that track sessions or preferences.
 * 
 * @param {OpenAPIDocument} openApi - Full OpenAPI specification
 * @param {string} path - API endpoint path
 * @param {string} method - HTTP method
 * @returns {HarParameterObject[]} Array of cookie parameters with example values
 * 
 * @example
 * getCookies(openApi, '/users/profile', 'get')
 * // → [{name: 'sessionId', value: 'abc123'}, {name: 'preferences', value: 'dark-mode'}]
 */
const getCookies = function (
    openApi: OpenAPIDocument,
    path: string,
    method: string
): HarParameterObject[] {
    return getParameterCollectionIn(openApi, path, method, 'cookie');
};

/**
 * Get all HTTP headers for an OpenAPI endpoint with security and accept headers
 * 
 * This comprehensive function builds the complete header list for an HTTP request
 * based on the OpenAPI specification. It handles:
 * 
 * 1. Accept Header (Content Negotiation)
 *    - From operation.consumes array (Swagger 2.0)
 *    - Lists MIME types the endpoint accepts
 *    - Example: 'application/json', 'text/xml', 'application/yaml'
 * 
 * 2. Header Parameters
 *    - Defined as parameters with in='header'
 *    - Custom headers required by the API
 *    - Example: 'X-API-Version: 2', 'X-Request-Id: abc123'
 * 
 * 3. Security Headers
 *    - Added based on security scheme definitions
 *    - Handles multiple security types:
 * 
 *    a) Basic Authentication (HTTP Basic Auth)
 *       - Header: Authorization: Basic <base64(username:password)>
 *       - Placeholder: Authorization: Basic REPLACE_BASIC_AUTH
 *    
 *    b) API Key Authentication
 *       - Header: {keyName}: {keyValue}
 *       - Only if in='header' (other locations use query/cookie)
 *       - Placeholder: X-API-Key: REPLACE_KEY_VALUE
 *    
 *    c) Bearer Token / OAuth2
 *       - Header: Authorization: Bearer <token>
 *       - Placeholder: Authorization: Bearer REPLACE_BEARER_TOKEN
 *       - Used for OAuth 2.0 and Bearer scheme HTTP
 * 
 * Security Resolution:
 * - Checks operation.security first (specific to this endpoint)
 * - Falls back to root security (default for whole API)
 * - Security scheme definitions found in:
 *   - OpenAPI 3.0: components.securitySchemes
 *   - Swagger 2.0: securityDefinitions
 * 
 * Priority for security headers:
 * 1. Basic Auth (if defined)
 * 2. API Key in header (if defined)
 * 3. Bearer/OAuth token (if defined)
 * 
 * Only one type of auth header is added, in this priority order.
 * 
 * @param {OpenAPIDocument} openApi - Full OpenAPI/Swagger specification
 * @param {string} path - API endpoint path
 * @param {string} method - HTTP method
 * @returns {HarParameterObject[]} Array of header objects with names and values
 * 
 * @example
 * // With API Key authentication
 * getHeadersArray(openApi, '/users', 'get')
 * // → [
 * //   {name: 'X-API-Key', value: 'REPLACE_KEY_VALUE'},
 * //   {name: 'Accept', value: 'application/json'}
 * // ]
 * 
 * // With Bearer token authentication
 * getHeadersArray(openApi, '/api/profile', 'get')
 * // → [
 * //   {name: 'Authorization', value: 'Bearer REPLACE_BEARER_TOKEN'},
 * //   {name: 'Accept', value: 'application/json'}
 * // ]
 * 
 * // With Basic authentication
 * getHeadersArray(openApi, '/admin', 'get')
 * // → [
 * //   {name: 'Authorization', value: 'Basic REPLACE_BASIC_AUTH'},
 * //   {name: 'Accept', value: 'application/json'}
 * // ]
 */
const getHeadersArray = function (
    openApi: OpenAPIDocument,
    path: string,
    method: string
): HarParameterObject[] {
    const headers: HarParameterObject[] = [];
    const pathObj = openApi.paths[path][method];
    if (!pathObj || typeof pathObj === 'object' && 'name' in pathObj) return headers;

    const methodObj = pathObj as OpenAPIOperation;

    // 'accept' header:
    if (typeof methodObj.consumes !== 'undefined') {
        for (let i in methodObj.consumes) {
            const type = methodObj.consumes[i];
            headers.push({
                name: 'accept',
                value: type,
            });
        }
    }

    // headers defined in path object:
    headers.push(...getParameterCollectionIn(openApi, path, method, 'header'));

    // security:
    let basicAuthDef: string | undefined;
    let apiKeyAuthDef: OpenAPISecurityScheme | undefined;
    let oauthDef: string | undefined;
    if (typeof methodObj.security !== 'undefined') {
        for (var l in methodObj.security) {
            const secScheme = Object.keys(methodObj.security[l])[0];
            const secDefinitions = openApi.securityDefinitions || openApi.components?.securitySchemes;
            if (!secDefinitions) continue;
            const secDefinition = secDefinitions[secScheme];
            if (!secDefinition) continue;
            const authType = secDefinition.type.toLowerCase();
            let authScheme = null;

            if (authType !== 'apikey' && secDefinition.scheme != null) {
                authScheme = secDefinition.scheme.toLowerCase();
            }

            switch (authType) {
                case 'basic':
                    basicAuthDef = secScheme;
                    break;
                case 'apikey':
                    if (secDefinition.in === 'header') {
                        apiKeyAuthDef = secDefinition;
                    }
                    break;
                case 'oauth2':
                    oauthDef = secScheme;
                    break;
                case 'http':
                    switch (authScheme) {
                        case 'bearer':
                            oauthDef = secScheme;
                            break;
                        case 'basic':
                            basicAuthDef = secScheme;
                            break;
                    }
                    break;
            }
        }
    } else if (typeof openApi.security !== 'undefined') {
        // Need to check OAS 3.0 spec about type http and scheme
        for (let m in openApi.security) {
            const secScheme = Object.keys(openApi.security[m])[0];
            const secDefinition = openApi.components?.securitySchemes?.[secScheme];
            if (!secDefinition) continue;
            const authType = secDefinition.type.toLowerCase();
            let authScheme = null;

            if (authType !== 'apikey' && authType !== 'oauth2') {
                authScheme = secDefinition.scheme?.toLowerCase();
            }

            switch (authType) {
                case 'http':
                    switch (authScheme) {
                        case 'bearer':
                            oauthDef = secScheme;
                            break;
                        case 'basic':
                            basicAuthDef = secScheme;
                            break;
                    }
                    break;
                case 'basic':
                    basicAuthDef = secScheme;
                    break;
                case 'apikey':
                    if (secDefinition.in === 'header') {
                        apiKeyAuthDef = secDefinition;
                    }
                    break;
                case 'oauth2':
                    oauthDef = secScheme;
                    break;
            }
        }
    }

    if (basicAuthDef) {
        headers.push({
            name: 'Authorization',
            value: 'Basic ' + 'REPLACE_BASIC_AUTH',
        });
    } else if (apiKeyAuthDef) {
        headers.push({
            name: apiKeyAuthDef.name || 'X-API-Key',
            value: 'REPLACE_KEY_VALUE',
        });
    } else if (oauthDef) {
        headers.push({
            name: 'Authorization',
            value: 'Bearer ' + 'REPLACE_BEARER_TOKEN',
        });
    }

    return headers;
};

/**
 * Convert entire OpenAPI specification to list of HAR endpoints
 * 
 * This is the main conversion function that transforms a complete OpenAPI or
 * Swagger specification into a list of HAR (HTTP Archive) endpoint objects.
 * 
 * Process:
 * 1. Iterate through all paths in the specification
 * 2. For each path, iterate through all HTTP methods (GET, POST, etc.)
 * 3. For each method, generate one or more HAR requests
 * 4. Build HarEndpoint object containing:
 *    - HTTP method (GET, POST, etc.)
 *    - Full URL
 *    - Description from operation
 *    - Array of HAR requests (one per content-type variant)
 * 5. Return complete list of endpoints
 * 
 * Result structure:
 * ```
 * [
 *   {
 *     method: 'GET',
 *     url: 'https://api.example.com/users',
 *     description: 'Get all users',
 *     hars: [
 *       {method: 'GET', url: 'https://api.example.com/users', headers: [...], ...}
 *     ]
 *   },
 *   {
 *     method: 'POST',
 *     url: 'https://api.example.com/users',
 *     description: 'Create a new user',
 *     hars: [
 *       {method: 'POST', url: 'https://api.example.com/users', postData: {...}, ...}
 *     ]
 *   },
 *   ...
 * ]
 * ```
 * 
 * @param {OpenAPIDocument} openApi - Full OpenAPI/Swagger specification
 * @returns {HarEndpoint[]} Array of HAR endpoint objects
 * 
 * @throws Returns empty array if error occurs during conversion (see console.log for error details)
 * 
 * @example
 * const endpoints = openApiToHarList(openApiSpec);
 * // endpoints[0] = {
 * //   method: 'GET',
 * //   url: 'https://api.example.com/users',
 * //   description: 'Retrieve a list of all users',
 * //   hars: [...]
 * // }
 */
const openApiToHarList = function (openApi: OpenAPIDocument): HarEndpoint[] {
    try {
        // iterate openApi and create har objects:
        const harList: HarEndpoint[] = [];
        for (let path in openApi.paths) {
            const pathObj = openApi.paths[path];
            for (let method in pathObj) {
                if (method === 'parameters' || method === 'servers' || method === '$ref') continue;
                const url = getBaseUrl(openApi, path, method) + path;
                const hars = createHar(openApi, path, method);
                // need to push multiple here
                harList.push({
                    method: method.toUpperCase(),
                    url: url,
                    description:
                        ((pathObj[method] as any)?.description) ||
                        'No description available',
                    hars: hars,
                });
            }
        }

        return harList;
    } catch (e) {
        console.log(e);
        return [];
    }
};

/**
 * Resolve a JSON reference ($ref) in the OpenAPI specification
 * 
 * OpenAPI uses JSON references to reduce duplication. Instead of defining
 * the same schema multiple times, you define it once and reference it.
 * 
 * Format: "#/components/schemas/User"
 * - '#' indicates reference is within this document (not external)
 * - '/' separates the path parts
 * - 'components', 'schemas', 'User' are the keys to follow
 * 
 * Algorithm:
 * 1. Split reference by '/' to get path parts
 * 2. Start at root OpenAPI object
 * 3. Navigate through each part: openApi['components']['schemas']['User']
 * 4. Return the value at the end of the path
 * 5. Return empty object if path is too short or reference is invalid
 * 
 * Example references:
 * - "#/components/schemas/User" → openApi.components.schemas.User
 * - "#/components/securitySchemes/OAuth2" → openApi.components.securitySchemes.OAuth2
 * - "#/parameters/PageParam" → openApi.parameters.PageParam (Swagger 2.0 style)
 * 
 * Note: This implementation only handles internal references (starting with #).
 * External references (e.g., "external.json#/User") are not supported.
 * 
 * @param {OpenAPIDocument} openApi - Full OpenAPI specification
 * @param {string} ref - JSON reference string (e.g., "#/components/schemas/User")
 * @returns {unknown} The referenced value, or empty object if not found
 * 
 * @example
 * // Reference to a schema
 * resolveRef(openApi, '#/components/schemas/User')
 * // → {type: 'object', properties: {id: {type: 'integer'}, name: {type: 'string'}}}
 * 
 * // Reference to a parameter
 * resolveRef(openApi, '#/parameters/PageParam')
 * // → {name: 'page', in: 'query', type: 'integer', description: 'Page number'}
 */
const resolveRef = function (openApi: OpenAPIDocument, ref: string): unknown {
    const parts = ref.split('/');

    if (parts.length <= 1) return {}; // = 3

    const recursive = function (obj: unknown, index: number): unknown {
        if (index + 1 < parts.length) {
            // index = 1
            let newCount = index + 1;
            return recursive((obj as Record<string, unknown>)[parts[index]], newCount);
        } else {
            return (obj as Record<string, unknown>)[parts[index]];
        }
    };
    return recursive(openApi, 1);
};

/**
 * Public API: Get all endpoints from OpenAPI specification
 * Alias for openApiToHarList function
 * @function
 */
export const getAll = openApiToHarList;

/**
 * Public API: Get HAR requests for a specific OpenAPI endpoint
 * Alias for createHar function
 * @function
 */
export const getEndpoint = createHar;