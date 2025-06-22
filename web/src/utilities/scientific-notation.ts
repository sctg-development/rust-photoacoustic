/**
 * @copyright Copyright (c) 2024-2025 Ronan LE MEILLAT
 * @license AGPL-3.0-or-later
 *
 * Scientific Notation Utility
 *
 * This utility provides comprehensive functions for converting numbers to scientific notation
 * in various formats (string, LaTeX, MathML, HTML). It uses engineering notation where
 * exponents are multiples of 3 for better readability in scientific contexts.
 *
 * Features:
 * - Engineering notation (exponents as multiples of 3: ..., -6, -3, 0, 3, 6, ...)
 * - Configurable precision for mantissa
 * - Multiple output formats for different rendering contexts
 * - Proper handling of edge cases (zero, very small/large numbers)
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU Affero General Public License as
 * published by the Free Software Foundation, either version 3 of the
 * License, or (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
 * GNU Affero General Public License for more details.
 *
 * You should have received a copy of the GNU Affero General Public License
 * along with this program. If not, see <http://www.gnu.org/licenses/>.
 */

/**
 * Represents a number in scientific notation with separate mantissa and exponent
 *
 * @interface ScientificNotationNumber
 * @property mantissa - The significant digits part (typically between 1 and 999.999...)
 * @property exponent - The power of 10, always a multiple of 3 (engineering notation)
 *
 * @example
 * // For the number 0.00123
 * // Result: { mantissa: 1.23, exponent: -3 }
 * // Represents: 1.23 × 10⁻³
 */
export type ScientificNotationNumber = {
  mantissa: number; // Mantissa (significant digits)
  exponent: number; // Exponent rounded to the nearest multiple of 3
};

/**
 * Utility class for converting numbers to scientific notation in various formats
 *
 * This class provides static methods to convert numerical values into scientific notation
 * using engineering notation (exponents as multiples of 3). This format is preferred
 * in engineering and scientific contexts as it aligns with SI unit prefixes.
 *
 * Engineering notation examples:
 * - 1234 → 1.234 × 10³ (not 1.234 × 10³)
 * - 0.00456 → 4.56 × 10⁻³ (not 4.56 × 10⁻³)
 * - 0.000789 → 789 × 10⁻⁶ (not 7.89 × 10⁻⁴)
 */
export class ScientificNotation {
  /**
   * Convert a number to scientific notation with engineering notation format
   *
   * This is the core conversion function that all other methods depend on.
   * It uses engineering notation where exponents are always multiples of 3,
   * which aligns with SI unit prefixes (milli-, micro-, kilo-, mega-, etc.).
   *
   * Algorithm:
   * 1. Handle special case of zero
   * 2. Calculate the natural logarithm base-10 exponent
   * 3. Round down to the nearest multiple of 3 for engineering notation
   * 4. Calculate mantissa by dividing by 10^(normalized_exponent)
   * 5. Round mantissa to specified precision
   *
   * @param value - The numerical value to convert
   * @param precision - Number of significant digits for the mantissa (default: 3)
   *                   Values < 1 are treated as 0 (no decimal places)
   * @returns Object containing mantissa and exponent in engineering notation
   *
   * @example
   * // Convert 0.00123 with default precision (3)
   * toScientificNotation(0.00123)
   * // Returns: { mantissa: 1.23, exponent: -3 }
   *
   * @example
   * // Convert 4567000 with high precision (6)
   * toScientificNotation(4567000, 6)
   * // Returns: { mantissa: 4.567, exponent: 6 }
   */
  public static toScientificNotation(
    value: number,
    precision?: number,
  ): ScientificNotationNumber {
    // Set default precision and validate input
    if (precision === undefined) {
      precision = 3;
    } else if (precision < 1) {
      precision = 0; // No decimal places
    } else {
      // Convert to decimal places (precision - 1)
      precision = Math.floor(precision - 1);
    }

    // Handle zero as a special case
    if (value === 0) {
      return { mantissa: 0, exponent: 0 };
    }

    // Calculate the natural exponent using logarithm base 10
    // Math.log10(Math.abs(value)) gives us the power of 10
    const exp = Math.floor(Math.log10(Math.abs(value)));

    // Convert to engineering notation by rounding down to nearest multiple of 3
    // This ensures exponents like: ..., -6, -3, 0, 3, 6, 9, ...
    const normalizedExp = Math.floor(exp / 3) * 3;

    // Calculate mantissa by dividing the original value by 10^(normalized_exponent)
    let mantissa = value / Math.pow(10, normalizedExp);

    // Round mantissa to the specified precision
    // Math.round(mantissa * 10^precision) / 10^precision
    mantissa = Math.round(mantissa * 10 ** precision) / 10 ** precision;

    return { mantissa, exponent: normalizedExp };
  }

  /**
   * Convert a number to scientific notation string in "e" format
   *
   * This format is commonly used in programming and calculator displays.
   * The "e" represents "times ten to the power of".
   *
   * @param value - The numerical value to convert
   * @param precision - Number of significant digits (optional, default: 3)
   * @returns String in format "mantissaEexponent" (e.g., "1.23e-3")
   *
   * @example
   * toScientificNotationString(0.00123) // Returns: "1.23e-3"
   * toScientificNotationString(4567000) // Returns: "4.567e6"
   */
  public static toScientificNotationString(
    value: number,
    precision?: number,
  ): string {
    const { mantissa, exponent } = this.toScientificNotation(value, precision);

    return `${mantissa}e${exponent}`;
  }

  /**
   * Convert a number to scientific notation LaTeX string
   *
   * LaTeX format is used for mathematical typesetting in documents and equations.
   * The \times command creates the multiplication symbol (×) and ^{} creates superscripts.
   *
   * @param value - The numerical value to convert
   * @param precision - Number of significant digits (optional, default: 3)
   * @returns LaTeX string in format "mantissa \times 10^{exponent}"
   *
   * @example
   * toScientificNotationLatex(0.00123) // Returns: "1.23 \\times 10^{-3}"
   * toScientificNotationLatex(4567000) // Returns: "4.567 \\times 10^{6}"
   */
  public static toScientificNotationLatex(
    value: number,
    precision?: number,
  ): string {
    const { mantissa, exponent } = this.toScientificNotation(value, precision);

    return `${mantissa} \\times 10^{${exponent}}`;
  }

  /**
   * Convert a number to scientific notation MathML string
   *
   * MathML (Mathematical Markup Language) is used for displaying mathematical
   * notation in web browsers. The <msup> element creates superscripts.
   *
   * @param value - The numerical value to convert
   * @param precision - Number of significant digits (optional, default: 3)
   * @returns MathML string with proper superscript formatting
   *
   * @example
   * toScientificNotationMathML(0.00123)
   * // Returns: "1.23 <msup><mn>10</mn><mn>-3</mn></msup>"
   */
  public static toScientificNotationMathML(
    value: number,
    precision?: number,
  ): string {
    const { mantissa, exponent } = this.toScientificNotation(value, precision);

    return `${mantissa} <msup><mn>10</mn><mn>${exponent}</mn></msup>`;
  }

  /**
   * Convert a number to scientific notation HTML string
   *
   * HTML format uses the <sup> tag for superscripts and the × symbol for multiplication.
   * This format is suitable for direct display in web pages without special rendering.
   *
   * @param value - The numerical value to convert
   * @param precision - Number of significant digits (optional, default: 3)
   * @returns HTML string with superscript formatting using <sup> tags
   *
   * @example
   * toScientificNotationHTML(0.00123) // Returns: "1.23 × 10<sup>-3</sup>"
   * toScientificNotationHTML(4567000) // Returns: "4.567 × 10<sup>6</sup>"
   */
  public static toScientificNotationHTML(
    value: number,
    precision?: number,
  ): string {
    const { mantissa, exponent } = this.toScientificNotation(value, precision);

    return `${mantissa} × 10<sup>${exponent}</sup>`;
  }
}
