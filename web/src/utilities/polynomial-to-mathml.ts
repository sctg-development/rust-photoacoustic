/**
 * @copyright Copyright (c) 2024-2025 Ronan LE MEILLAT
 * @license AGPL-3.0-or-later
 *
 * Polynomial to LaTeX and MathML Utility
 *
 * This utility provides functions to convert polynomial coefficients into
 * properly formatted LaTeX and MathML for display in web browsers. It handles
 * scientific notation, proper sign formatting, and zero coefficient filtering.
 *
 * Functions:
 * - getMathMLFromPolynomialCoefficients: Generates LaTeX representation
 * - getMathMLFromPolynomialCoefficientsClassicOrder: Generates LaTeX in classic order
 * - getMathMLFromPolynomialCoefficientsMathML: Converts LaTeX to MathML
 * - getMathMLFromPolynomialCoefficientsClassicOrderMathML: Converts classic LaTeX to MathML
 */

import Temml from "temml";

import { ScientificNotation } from "./scientific-notation";

/**
 * Converts an array of polynomial coefficients to LaTeX format
 *
 * This function takes an array of n coefficients representing a polynomial of degree n
 * in the form: a₀ + a₁x + a₂x² + a₃x³ + a₄x⁴
 *
 * Features:
 * - Skips zero coefficients to create clean expressions
 * - Handles positive and negative signs correctly
 * - Uses scientific notation for coefficients when appropriate
 * - Generates proper LaTeX syntax for mathematical rendering
 *
 * @param coefficients - Array of exactly n numerical coefficients example for 5 [a₀, a₁, a₂, a₃, a₄]
 *                      where index 0 is the constant term and index 4 is the highest degree term
 * @returns LaTeX string representation of the polynomial, or fallback string if invalid input
 *
 * @example
 * // For coefficients [1.5e-3, 0, -2.1, 0, 4.7e-6]
 * // Returns LaTeX for: 1.5×10⁻³ - 2.1x² + 4.7×10⁻⁶x⁴
 * const latex = getMathMLFromPolynomialCoefficients([1.5e-3, 0, -2.1, 0, 4.7e-6]);
 *
 * @example
 * // For all zero coefficients [0, 0, 0, 0, 0]
 * // Returns LaTeX for: 0
 * const latex = getMathMLFromPolynomialCoefficients([0, 0, 0, 0, 0]);
 */
export const getMathMLFromPolynomialCoefficients = (coefficients: number[]) => {
  try {
    // Step 1: Process each coefficient and create term objects
    // This maps coefficients to their LaTeX representation with proper formatting
    const terms = coefficients
      .map((coeff, index) => {
        // Skip zero coefficients to avoid cluttering the expression
        if (coeff === 0) {
          return null;
        }

        // Convert coefficient to scientific notation LaTeX format
        const coeffLatex = ScientificNotation.toScientificNotationLatex(coeff);

        // Generate the power part of the term based on the index
        // index 0: constant term (no x)
        // index 1: linear term (x)
        // index 2+: higher powers (x², x³, etc.)
        const powerStr = index === 0 ? "" : index === 1 ? "x" : `x^${index}`;

        return {
          coefficient: coeff,
          // Combine coefficient and power with multiplication symbol if needed
          latex: `${coeffLatex}${powerStr ? `\\cdot ${powerStr}` : ""}`,
          isNegative: coeff < 0,
        };
      })
      // Remove null entries (zero coefficients)
      .filter((term) => term !== null);

    // Handle edge case: all coefficients are zero
    if (terms.length === 0) {
      return "0";
    }

    // Step 2: Build the polynomial string with proper sign handling
    let polynomial = "";

    for (let i = 0; i < terms.length; i++) {
      const term = terms[i];

      if (i === 0) {
        // First term: display as-is (positive or negative)
        // No leading + sign for positive first terms
        polynomial = term.latex;
      } else {
        // Subsequent terms: handle signs between terms
        if (term.isNegative) {
          // Negative coefficient already includes minus sign
          // Just add a space before the term
          polynomial += ` ${term.latex}`;
        } else {
          // Positive coefficient needs explicit + sign
          polynomial += ` + ${term.latex}`;
        }
      }
    }

    // Step 3: Return LaTeX string
    return polynomial;
  } catch (error) {
    // Handle any errors during LaTeX generation
    console.error("Error generating polynomial LaTeX:", error);

    // Fallback: return a simple bracketed array representation
    // This ensures the UI doesn't break if LaTeX generation fails
    return `[${coefficients.join(", ")}]`;
  }
};

/**
 * Converts an array of polynomial coefficients to LaTeX format in classical mathematical order
 *
 * This function is similar to getMathMLFromPolynomialCoefficients but displays the polynomial
 * in the traditional mathematical order: from highest degree term to constant term.
 * For a degree-4 polynomial: a₄x⁴ + a₃x³ + a₂x² + a₁x + a₀
 *
 * The input array is still indexed from constant to highest degree [a₀, a₁, a₂, a₃, a₄],
 * but the output displays terms in reverse order for conventional mathematical presentation.
 *
 * Features:
 * - Displays terms in decreasing order of powers (x⁴, x³, x², x, constant)
 * - Skips zero coefficients to create clean expressions
 * - Handles positive and negative signs correctly
 * - Uses scientific notation for coefficients when appropriate
 * - Generates proper LaTeX syntax for mathematical rendering
 *
 * @param coefficients - Array of exactly 5 numerical coefficients [a₀, a₁, a₂, a₃, a₄]
 *                      where index 0 is the constant term and index 4 is the highest degree term
 * @returns LaTeX string representation of the polynomial in classical order, or null if invalid input
 *
 * @example
 * // For coefficients [1.5e-3, 0, -2.1, 0, 4.7e-6]
 * // Returns LaTeX for: 4.7×10⁻⁶x⁴ - 2.1x² + 1.5×10⁻³
 * const latex = getMathMLFromPolynomialCoefficientsClassicOrder([1.5e-3, 0, -2.1, 0, 4.7e-6]);
 *
 * @example
 * // For coefficients [5, -3, 0, 2, -1]
 * // Returns LaTeX for: -x⁴ + 2x³ - 3x + 5
 * const latex = getMathMLFromPolynomialCoefficientsClassicOrder([5, -3, 0, 2, -1]);
 */
export const getMathMLFromPolynomialCoefficientsClassicOrder = (
  coefficients: number[],
) => {
  // Validate input: must be exactly 5 coefficients for a degree-4 polynomial
  if (coefficients.length !== 5) {
    return null;
  }

  try {
    // Step 1: Process each coefficient and create term objects
    // Note: We process in reverse order to display from highest to lowest degree
    const terms = coefficients
      .map((coeff, index) => {
        // Skip zero coefficients to avoid cluttering the expression
        if (coeff === 0) {
          return null;
        }

        // Convert coefficient to scientific notation LaTeX format
        const coeffLatex = ScientificNotation.toScientificNotationLatex(coeff);

        // Generate the power part of the term based on the index
        // index 0: constant term (no x)
        // index 1: linear term (x)
        // index 2+: higher powers (x², x³, etc.)
        const powerStr = index === 0 ? "" : index === 1 ? "x" : `x^${index}`;

        return {
          coefficient: coeff,
          // Combine coefficient and power with multiplication symbol if needed
          latex: `${coeffLatex}${powerStr ? `\\cdot ${powerStr}` : ""}`,
          isNegative: coeff < 0,
          index: index,
        };
      })
      // Remove null entries (zero coefficients)
      .filter((term) => term !== null)
      // Sort in descending order of powers for classical mathematical presentation
      .sort((a, b) => b.index - a.index);

    // Handle edge case: all coefficients are zero
    if (terms.length === 0) {
      return "0";
    }

    // Step 2: Build the polynomial string with proper sign handling
    // Starting from highest degree and working down to constant
    let polynomial = "";

    for (let i = 0; i < terms.length; i++) {
      const term = terms[i];

      if (i === 0) {
        // First term (highest degree): display as-is (positive or negative)
        // No leading + sign for positive first terms
        polynomial = term.latex;
      } else {
        // Subsequent terms: handle signs between terms
        if (term.isNegative) {
          // Negative coefficient already includes minus sign
          // Just add a space before the term
          polynomial += ` ${term.latex}`;
        } else {
          // Positive coefficient needs explicit + sign
          polynomial += ` + ${term.latex}`;
        }
      }
    }

    // Step 3: Return LaTeX string
    return polynomial;
  } catch (error) {
    // Handle any errors during LaTeX generation
    console.error("Error generating polynomial LaTeX in classic order:", error);

    // Fallback: return a simple bracketed array representation
    // This ensures the UI doesn't break if LaTeX generation fails
    return `[${coefficients.join(", ")}]`;
  }
};

/**
 * Converts an array of polynomial coefficients to MathML format using Temml
 *
 * This function takes an array of n coefficients representing a polynomial of degree n
 * in the form: a₀ + a₁x + a₂x² + a₃x³ + a₄x⁴ and converts it to MathML.
 *
 * @param coefficients - Array of numerical coefficients
 * @returns MathML string representation of the polynomial
 *
 * @example
 * // For coefficients [1.5e-3, 0, -2.1, 0, 4.7e-6]
 * // Returns MathML for: 1.5×10⁻³ - 2.1x² + 4.7×10⁻⁶x⁴
 * const mathml = getMathMLFromPolynomialCoefficientsMathML([1.5e-3, 0, -2.1, 0, 4.7e-6]);
 */
export const getMathMLFromPolynomialCoefficientsMathML = (
  coefficients: number[],
) => {
  try {
    const latex = getMathMLFromPolynomialCoefficients(coefficients);

    return Temml.renderToString(latex);
  } catch (error) {
    console.error("Error rendering polynomial to MathML:", error);

    return `[${coefficients.join(", ")}]`;
  }
};

/**
 * Converts an array of polynomial coefficients to MathML format in classical mathematical order using Temml
 *
 * This function displays the polynomial in the traditional mathematical order: from highest degree term to constant term.
 * For a degree-4 polynomial: a₄x⁴ + a₃x³ + a₂x² + a₁x + a₀ and converts it to MathML.
 *
 * @param coefficients - Array of exactly 5 numerical coefficients [a₀, a₁, a₂, a₃, a₄]
 * @returns MathML string representation of the polynomial in classical order
 *
 * @example
 * // For coefficients [1.5e-3, 0, -2.1, 0, 4.7e-6]
 * // Returns MathML for: 4.7×10⁻⁶x⁴ - 2.1x² + 1.5×10⁻³
 * const mathml = getMathMLFromPolynomialCoefficientsClassicOrderMathML([1.5e-3, 0, -2.1, 0, 4.7e-6]);
 */
export const getMathMLFromPolynomialCoefficientsClassicOrderMathML = (
  coefficients: number[],
) => {
  try {
    const latex = getMathMLFromPolynomialCoefficientsClassicOrder(coefficients);

    if (latex === null) {
      return null;
    }

    return Temml.renderToString(latex);
  } catch (error) {
    console.error(
      "Error rendering polynomial to MathML in classic order:",
      error,
    );

    return `[${coefficients.join(", ")}]`;
  }
};
