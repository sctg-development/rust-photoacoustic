/**
 * @copyright Copyright (c) 2024-2025 Ronan LE MEILLAT
 * @license AGPL-3.0-or-later
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
import { memo, useState, useEffect, useCallback } from "react";
import { type ButtonProps } from "@heroui/react";

import { CheckLinearIcon, CopyLinearIcon } from "../components/icons";

import { PreviewButton } from "./preview-button";

export interface CopyButtonProps extends Omit<ButtonProps, "value"> {
  value?: string;
  copiedTimeout?: number;
  onCopySuccess?: () => void;
  onCopyError?: (error: unknown) => void;
}

export const CopyButton = memo<CopyButtonProps>(
  ({
    value,
    className,
    copiedTimeout = 2000,
    onCopySuccess,
    onCopyError,
    ...buttonProps
  }) => {
    const [isCopied, setIsCopied] = useState(false);
    const [hasCopyError, setHasCopyError] = useState(false);

    useEffect(() => {
      if (hasCopyError) {
        const timer = setTimeout(() => setHasCopyError(false), copiedTimeout);

        return () => clearTimeout(timer);
      }
    }, [hasCopyError, copiedTimeout]);

    const handleCopy = useCallback(async () => {
      try {
        if (!value) throw new Error("No value to copy");
        await navigator.clipboard.writeText(value);
        setIsCopied(true);
        if (onCopySuccess) onCopySuccess();
        setTimeout(() => setIsCopied(false), copiedTimeout);
      } catch (error) {
        setHasCopyError(true);
        if (onCopyError) onCopyError(error);
      }
    }, [value, onCopySuccess, onCopyError, copiedTimeout]);

    const icon = isCopied ? (
      <CheckLinearIcon
        className="opacity-0 scale-50 data-[visible=true]:opacity-100 data-[visible=true]:scale-100 transition-transform-opacity"
        data-visible={isCopied}
        size={16}
      />
    ) : (
      <CopyLinearIcon
        className="opacity-0 scale-50 data-[visible=true]:opacity-100 data-[visible=true]:scale-100 transition-transform-opacity"
        data-visible={!isCopied && !hasCopyError}
        size={16}
      />
    );

    return (
      <PreviewButton
        className={className ?? "-top-1 left-0.5"}
        icon={icon}
        onPress={handleCopy}
        {...buttonProps}
      />
    );
  },
);

CopyButton.displayName = "CopyButton";
