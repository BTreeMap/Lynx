import React, { useEffect } from 'react';
import { createPortal } from 'react-dom';
import { X } from 'lucide-react';
import { cn } from '../../lib/cn';
import { IconButton } from './IconButton';

export interface DialogProps {
    open: boolean;
    onClose: () => void;
    title?: React.ReactNode;
    description?: React.ReactNode;
    children: React.ReactNode;
    footer?: React.ReactNode;
    className?: string;
}

export const Dialog: React.FC<DialogProps> = ({
    open,
    onClose,
    title,
    description,
    children,
    footer,
    className,
}) => {
    useEffect(() => {
        if (!open) return;
        const onKey = (event: KeyboardEvent) => {
            if (event.key === 'Escape') onClose();
        };
        document.addEventListener('keydown', onKey);
        const original = document.body.style.overflow;
        document.body.style.overflow = 'hidden';
        return () => {
            document.removeEventListener('keydown', onKey);
            document.body.style.overflow = original;
        };
    }, [open, onClose]);

    if (!open) return null;

    return createPortal(
        <div
            className="fixed inset-0 z-50 flex items-center justify-center p-4 animate-fade-in"
            role="dialog"
            aria-modal="true"
        >
            <div
                className="absolute inset-0 bg-baltic-blue-950/55 backdrop-blur-sm"
                onClick={onClose}
                aria-hidden
            />
            <div
                className={cn(
                    'relative z-10 w-full max-w-lg animate-scale-in rounded-2xl border border-border bg-elevated shadow-elevated',
                    className,
                )}
            >
                {(title || description) && (
                    <div className="flex items-start justify-between gap-4 border-b border-border p-5 sm:p-6">
                        <div className="space-y-1">
                            {title && <h3 className="text-lg font-semibold tracking-tight text-fg">{title}</h3>}
                            {description && <p className="text-sm text-fg-muted">{description}</p>}
                        </div>
                        <IconButton label="Close dialog" onClick={onClose} className="-mt-1 -mr-1 shrink-0">
                            <X className="h-5 w-5" />
                        </IconButton>
                    </div>
                )}
                <div className="p-5 sm:p-6">{children}</div>
                {footer && (
                    <div className="flex flex-col-reverse gap-3 border-t border-border p-5 sm:flex-row sm:justify-end sm:p-6">
                        {footer}
                    </div>
                )}
            </div>
        </div>,
        document.body,
    );
};
