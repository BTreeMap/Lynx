import React, { useState } from 'react';
import { CheckCircle2, Link2, Plus, Sparkles } from 'lucide-react';
import { shortCodeMaxLength } from '../auth/model';
import { useAuth } from '../hooks/useAuth';
import { apiClient } from '../api';
import type { ShortenedUrl } from '../types';
import { buildShortLink } from '../utils/url';
import { extractErrorMessage } from '../utils/errorHandling';
import { Button } from './ui/Button';
import { Card, CardBody, CardHeader, CardTitle, CardDescription } from './ui/Card';
import { Field, Input } from './ui/Input';
import { Alert } from './ui/Alert';
import { Dialog } from './ui/Dialog';
import { CopyButton } from './ui/CopyButton';

interface CreateUrlFormProps {
    /** Called once, after a link has actually been created. */
    onUrlCreated: () => void;
}

/**
 * Submission as a closed set of states.
 *
 * The four fields this replaces (`isSubmitting`, `error`, `created`, `successLink`,
 * `showModal`) had combinations the flow never reaches — "submitting while showing the
 * success dialog", "a dialog open with nothing to show" — and the dialog's own
 * visibility had to be guarded with `showModal && !!displayValue` because the value it
 * renders lived in a different field from the flag that revealed it. Here the created
 * link *is* the state that opens the dialog.
 */
type CreatePhase =
    | { readonly tag: 'editing'; readonly error: string | null }
    | { readonly tag: 'submitting' }
    | { readonly tag: 'created'; readonly url: ShortenedUrl; readonly link: string | null };

const EDITING: CreatePhase = { tag: 'editing', error: null };

const CreateUrlForm: React.FC<CreateUrlFormProps> = ({ onUrlCreated }) => {
    const { state } = useAuth();
    const maxShortCodeLength = shortCodeMaxLength(state);

    const [url, setUrl] = useState('');
    const [customCode, setCustomCode] = useState('');
    const [phase, setPhase] = useState<CreatePhase>(EDITING);

    const handleSubmit = async (event: React.FormEvent) => {
        event.preventDefault();
        setPhase({ tag: 'submitting' });

        try {
            const created = await apiClient.createUrl({
                url,
                custom_code: customCode || undefined,
            });
            setUrl('');
            setCustomCode('');
            setPhase({
                tag: 'created',
                url: created,
                link: buildShortLink(created.short_code, created.redirect_base_url),
            });
            onUrlCreated();
        } catch (err: unknown) {
            setPhase({ tag: 'editing', error: extractErrorMessage(err, 'Failed to create URL') });
        }
    };

    const isSubmitting = phase.tag === 'submitting';

    return (
        <Card>
            <CardHeader>
                <CardTitle>Create a short link</CardTitle>
                <CardDescription>
                    Paste a long URL and optionally choose a custom code.
                </CardDescription>
            </CardHeader>
            <CardBody>
                <form onSubmit={handleSubmit} className="space-y-4 sm:space-y-5">
                    <div className="grid gap-4 sm:grid-cols-3 sm:items-start sm:gap-5">
                        <Field label="Original URL" htmlFor="url" required className="sm:col-span-2">
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

                    {phase.tag === 'editing' && phase.error && (
                        <Alert tone="error">{phase.error}</Alert>
                    )}

                    <div className="flex justify-stretch sm:justify-end">
                        <Button
                            type="submit"
                            isLoading={isSubmitting}
                            leftIcon={<Plus className="h-4 w-4" />}
                            className="w-full sm:w-auto"
                        >
                            {isSubmitting ? 'Creating…' : 'Create short link'}
                        </Button>
                    </div>
                </form>
            </CardBody>

            {/* Mounted only in the `created` phase, so its contents are read from a link
                that definitely exists rather than from nullables guarded at the door. */}
            {phase.tag === 'created' && (
                <Dialog
                    open
                    onClose={() => setPhase(EDITING)}
                    title={
                        <span className="flex items-center gap-2">
                            <CheckCircle2 className="h-5 w-5 text-success" />
                            Short link created
                        </span>
                    }
                    description="Your link is ready to share."
                    footer={
                        <>
                            <Button variant="secondary" onClick={() => setPhase(EDITING)}>
                                Done
                            </Button>
                            <CopyButton
                                value={phase.link ?? phase.url.short_code}
                                size="md"
                                variant="primary"
                                idleLabel="Copy link"
                            />
                        </>
                    }
                >
                    <div className="flex items-center gap-2.5 rounded-xl border border-border bg-surface-2/60 p-3.5 sm:gap-3 sm:p-4">
                        <span className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-primary-soft text-primary-soft-fg">
                            {phase.link ? (
                                <Link2 className="h-4.5 w-4.5" />
                            ) : (
                                <Sparkles className="h-4.5 w-4.5" />
                            )}
                        </span>
                        {phase.link ? (
                            <a
                                href={phase.link}
                                target="_blank"
                                rel="noopener noreferrer"
                                className="min-w-0 break-all text-sm font-medium text-primary hover:underline"
                            >
                                {phase.link}
                            </a>
                        ) : (
                            <span className="min-w-0 break-all font-mono text-sm font-medium text-fg">
                                {phase.url.short_code}
                            </span>
                        )}
                    </div>
                </Dialog>
            )}
        </Card>
    );
};

export default CreateUrlForm;
