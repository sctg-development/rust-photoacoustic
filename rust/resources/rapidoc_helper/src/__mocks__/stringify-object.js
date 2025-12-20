/**
 * Mock for stringify-object - used when JSON.stringify is not suitable
 * This is a simple replacement for the ESM module
 */

module.exports = function stringifyObject(obj, options = {}) {
    const indent = options.indent || '  ';
    const singleQuotes = options.singleQuotes !== false;
    const quote = singleQuotes ? "'" : '"';

    function stringify(value, depth = 0) {
        const currentIndent = indent.repeat(depth);
        const nextIndent = indent.repeat(depth + 1);

        if (value === null) return 'null';
        if (value === undefined) return 'undefined';
        if (typeof value === 'string') return quote + value.replace(/\\/g, '\\\\').replace(new RegExp(quote, 'g'), '\\' + quote) + quote;
        if (typeof value === 'number' || typeof value === 'boolean') return String(value);
        if (value instanceof Date) return 'new Date(' + value.getTime() + ')';
        if (Array.isArray(value)) {
            const items = value.map(item => nextIndent + stringify(item, depth + 1));
            return '[\n' + items.join(',\n') + '\n' + currentIndent + ']';
        }
        if (typeof value === 'object') {
            const keys = Object.keys(value);
            const items = keys.map(key => {
                const k = /^[a-zA-Z_$][a-zA-Z0-9_$]*$/.test(key) ? key : quote + key + quote;
                return nextIndent + k + ': ' + stringify(value[key], depth + 1);
            });
            return '{\n' + items.join(',\n') + '\n' + currentIndent + '}';
        }
        return String(value);
    }

    return stringify(obj);
};

module.exports.default = module.exports;
