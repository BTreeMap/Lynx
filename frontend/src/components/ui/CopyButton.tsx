import React, { useCallback, useRef, useState } from 'react';
import { Check, Copy } from 'lucide-react';
import { Button, type ButtonProps } from './Button';

export interface CopyButtonProps extends Omit<ButtonProps, 'children' | 'onClick'> {
    value: string;
    idleLabel?: string;
    copiedLabel?: string;
    /** Hide the text label and show only the icon. */
    iconOnly?: boolean;
}

/** Button that copies a string to the clipboard with success feedback. */
export const CopyButton: React.FC<CopyButtonProps> = ({
    value,
    idleLabel = 'Copy',
    copiedLabel = 'Copied',
    iconOnly = false,
    variant = 'secondary',
    size = 'sm',
    ...props
}) => {
    const [copied, setCopied] = useState(false);
    const timer = useRef<ReturnType<typeof setTimeout> | null>(null);

    const handleCopy = useCallback(async () => {
        try {
            await navigator.clipboard.writeText(value);
            setCopied(true);
            if (timer.current) clearTimeout(timer.current);
            timer.current = setTimeout(() => setCopied(false), 2000);
        } catch (err) {
            console.error('Failed to copy:', err);
        }
    }, [value]);

    const icon = copied ? (
        <Check className="h-4 w-4 text-success" />
    ) : (
        <Copy className="h-4 w-4" />
    );

    return (
        <Button
            type="button"
            variant={variant}
            size={size}
            onClick={handleCopy}
            leftIcon={icon}
            aria-label={copied ? copiedLabel : idleLabel}
            {...props}
        >
            {!iconOnly && (copied ? copiedLabel : idleLabel)}
        </Button>
    );
};
