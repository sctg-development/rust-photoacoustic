// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

/**
 * Type declarations for stringify-object package
 * This is a simple wrapper to satisfy TypeScript's type checking
 */

declare module 'stringify-object' {
    interface StringifyOptions {
        indent?: string;
        singleQuotes?: boolean;
        filter?: (obj: any, prop: string) => any;
        transform?: (obj: any, prop: string, originalValue: any) => any;
        inlineCharacterLimit?: number;
    }

    function stringifyObject(value: any, options?: StringifyOptions): string;

    export = stringifyObject;
}
