import React, { useState } from 'react';
import { CheckCircle2, Link2, Plus, Sparkles } from 'lucide-react';
import { useAuth } from '../hooks/useAuth';
import { apiClient } from '../api';
import type { CreateUrlRequest, ShortenedUrl } from '../types';
import { buildShortLink } from '../utils/url';
import { extractErrorMessage } from '../utils/errorHandling';
import { Button } from './ui/Button';
import { Card, CardBody, CardHeader, CardTitle, CardDescription } from './ui/Card';
import { Field, Input } from './ui/Input';
import { Alert } from './ui/Alert';
import { Dialog } from './ui/Dialog';
import { CopyButton } from './ui/CopyButton';

const DEFAULT_SHORT_CODE_MAX_LENGTH = 50;

interface CreateUrlFormProps {
    onUrlCreated: () => void;
}

const CreateUrlForm: React.FC<CreateUrlFormProps> = ({ onUrlCreated }) => {
    const { shortCodeMaxLength } = useAuth();
    const maxShortCodeLength = shortCodeMaxLength || DEFAULT_SHORT_CODE_MAX_LENGTH;
    const [url, setUrl] = useState('');
    const [customCode, setCustomCode] = useState('');
    const [isSubmitting, setIsSubmitting] = useState(false);
    const [error, setError] = useState<string | null>(null);
    const [created, setCreated] = useState<ShortenedUrl | null>(null);
    const [successLink, setSuccessLink] = useState<string | null>(null);
    const [showModal, setShowModal] = useState(false);

    const handleSubmit = async (e: React.FormEvent) => {
        e.preventDefault();
        setIsSubmitting(true);
        setError(null);

        try {
            const request: CreateUrlRequest = {
                url,
                custom_code: customCode || undefined,
            };
            const result = await apiClient.createUrl(request);
            const fullLink = buildShortLink(result.short_code, result.redirect_base_url);
            setCreated(result);
            setSuccessLink(fullLink);
            setUrl('');
            setCustomCode('');
            setShowModal(true);
            onUrlCreated();
        } catch (err: unknown) {
            setError(extractErrorMessage(err, 'Failed to create URL'));
        } finally {
            setIsSubmitting(false);
        }
    };

    const handleDismiss = () => {
        setShowModal(false);
    };

    const displayValue = successLink ?? created?.short_code ?? '';

    return (
        <Card>
            <CardHeader>
                <CardTitle>Create a short link</CardTitle>
                <CardDescription>
                    Paste a long URL and optionally choose a custom code.
                </CardDescription>
            </CardHeader>
            <CardBody>
                <form onSubmit={handleSubmit} className="space-y-5">
                    <div className="grid gap-5 sm:grid-cols-[1fr_auto] sm:items-start">
                        <Field label="Original URL" htmlFor="url" required>
                            <Input
                                id="url"
                                type="url"
                                value={url}
                                onChange={(e) => setUrl(e.target.value)}
                                placeholder="https://example.com/very/long/url"
                                required
                            />
                        </Field>

                        <Field
                            label="Custom code"
                            htmlFor="customCode"
                            hint={`Up to ${maxShortCodeLength} characters. Optional.`}
                            className="sm:w-56"
                        >
                            <Input
                                id="customCode"
                                type="text"
                                value={customCode}
                                onChange={(e) => setCustomCode(e.target.value)}
                                placeholder="my-link"
                                maxLength={maxShortCodeLength}
                            />
                        </Field>
                    </div>

                    {error && <Alert tone="error">{error}</Alert>}

                    <div className="flex justify-end">
                        <Button
                            type="submit"
                            isLoading={isSubmitting}
                            leftIcon={<Plus className="h-4 w-4" />}
                        >
                            {isSubmitting ? 'Creating…' : 'Create short link'}
                        </Button>
                    </div>
                </form>
            </CardBody>

            <Dialog
                open={showModal && !!displayValue}
                onClose={handleDismiss}
                title={
                    <span className="flex items-center gap-2">
                        <CheckCircle2 className="h-5 w-5 text-success" />
                        Short link created
                    </span>
                }
                description="Your link is ready to share."
                footer={
                    <>
                        <Button variant="secondary" onClick={handleDismiss}>
                            Done
                        </Button>
                        <CopyButton value={displayValue} size="md" variant="primary" idleLabel="Copy link" />
                    </>
                }
            >
                <div className="flex items-center gap-3 rounded-xl border border-border bg-surface-2/60 p-4">
                    <span className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-primary-soft text-primary-soft-fg">
                        {successLink ? <Link2 className="h-4.5 w-4.5" /> : <Sparkles className="h-4.5 w-4.5" />}
                    </span>
                    {successLink ? (
                        <a
                            href={successLink}
                            target="_blank"
                            rel="noopener noreferrer"
                            className="min-w-0 break-all text-sm font-medium text-primary hover:underline"
                        >
                            {successLink}
                        </a>
                    ) : (
                        <span className="min-w-0 break-all font-mono text-sm font-medium text-fg">
                            {created?.short_code}
                        </span>
                    )}
                </div>
            </Dialog>
        </Card>
    );
};

export default CreateUrlForm;
