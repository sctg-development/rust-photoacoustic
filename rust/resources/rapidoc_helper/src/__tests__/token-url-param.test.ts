// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

/**
 * Tests for the URL `token` query-parameter → BearerAuth pre-fill feature.
 *
 * The logic lives in `index.ts` and cannot be imported as a module directly
 * (it has top-level side-effects that require a browser DOM).  We therefore
 * test the pure helper functions extracted here rather than the full module.
 */

/**
 * Extract the `token` value from a URL search string, mirroring the logic in
 * index.ts:
 *   const urlToken = new URLSearchParams(window.location.search).get('token');
 */
function extractTokenFromSearch(search: string): string | null {
    return new URLSearchParams(search).get('token');
}

/**
 * Simulate whether a `setApiKey` call would be made given a token string.
 * Returns the arguments that would be passed, or null if no call would happen.
 */
function simulateBearerInjection(
    token: string | null,
    rapidocElPresent: boolean,
): { schemeId: string; value: string } | null {
    if (token && rapidocElPresent) {
        return { schemeId: 'BearerAuth', value: token };
    }
    return null;
}

describe('URL token parameter extraction', () => {
    it('extracts a token from a plain query string', () => {
        expect(extractTokenFromSearch('?token=eyJhbGciOiJIUzI1NiJ9')).toBe('eyJhbGciOiJIUzI1NiJ9');
    });

    it('extracts a token when other parameters are also present', () => {
        expect(extractTokenFromSearch('?foo=bar&token=mytoken123&baz=qux')).toBe('mytoken123');
    });

    it('returns null when no token parameter is present', () => {
        expect(extractTokenFromSearch('?foo=bar')).toBeNull();
    });

    it('returns null for an empty search string', () => {
        expect(extractTokenFromSearch('')).toBeNull();
    });

    it('handles a token that contains special characters (URL-encoded)', () => {
        const raw = 'eyJ.abc+def/ghi==';
        const encoded = encodeURIComponent(raw);
        expect(extractTokenFromSearch(`?token=${encoded}`)).toBe(raw);
    });
});

describe('BearerAuth injection simulation', () => {
    it('injects the token into BearerAuth when token is present and element exists', () => {
        const result = simulateBearerInjection('mytoken', true);
        expect(result).toEqual({ schemeId: 'BearerAuth', value: 'mytoken' });
    });

    it('does not inject when token is null', () => {
        expect(simulateBearerInjection(null, true)).toBeNull();
    });

    it('does not inject when rapidoc element is absent', () => {
        expect(simulateBearerInjection('mytoken', false)).toBeNull();
    });

    it('does not inject when both token is null and element is absent', () => {
        expect(simulateBearerInjection(null, false)).toBeNull();
    });

    it('passes the raw token value (RapiDoc prepends "Bearer " internally)', () => {
        const token = 'eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9';
        const result = simulateBearerInjection(token, true);
        // The value must NOT already contain "Bearer " — RapiDoc adds it
        expect(result?.value).toBe(token);
        expect(result?.value).not.toMatch(/^Bearer /);
    });
});
