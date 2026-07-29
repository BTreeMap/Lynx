import React from 'react';
import { assertNever } from '../../lib/assertNever';
import { Alert } from '../ui/Alert';
import { Button } from '../ui/Button';
import { Dialog } from '../ui/Dialog';
import { Field, Input } from '../ui/Input';
import { editorBusy, editorError, type DestinationEditor } from './destinationEditor';

/**
 * The edit and restore dialogs.
 *
 * Exactly one is mounted, chosen by the editor's own state, so each renders from a
 * variant that carries everything it displays — the draft, or the entry being restored.
 * Nothing is read through a nullable that "should" be set by the time the dialog is
 * visible.
 */
export const DestinationDialogs: React.FC<{ readonly editor: DestinationEditor }> = ({
    editor,
}) => {
    const { state } = editor;
    if (state.tag === 'closed') {
        return null;
    }

    const busy = editorBusy(state);
    const error = editorError(state);

    switch (state.tag) {
        case 'editing':
            return (
                <Dialog
                    open
                    onClose={editor.dismiss}
                    title="Edit destination"
                    description="Update where this short link points. The previous destination is saved to history."
                    footer={
                        <>
                            <Button variant="secondary" onClick={editor.dismiss} disabled={busy}>
                                Cancel
                            </Button>
                            <Button
                                onClick={editor.submit}
                                isLoading={busy}
                                disabled={state.draft.trim().length === 0}
                            >
                                Save destination
                            </Button>
                        </>
                    }
                >
                    <div className="space-y-4">
                        <Field label="Destination URL" htmlFor="edit-destination">
                            <Input
                                id="edit-destination"
                                value={state.draft}
                                onChange={(event) => editor.changeDraft(event.target.value)}
                                placeholder="https://example.com/page"
                                invalid={Boolean(error)}
                                autoFocus
                                onKeyDown={(event) => {
                                    if (event.key === 'Enter') {
                                        event.preventDefault();
                                        editor.submit();
                                    }
                                }}
                            />
                        </Field>
                        {error && <Alert tone="error">{error}</Alert>}
                    </div>
                </Dialog>
            );

        case 'restoring':
            return (
                <Dialog
                    open
                    onClose={editor.dismiss}
                    title="Restore destination"
                    description="This makes the selected previous destination the link's active target. The current destination is saved to history."
                    footer={
                        <>
                            <Button variant="secondary" onClick={editor.dismiss} disabled={busy}>
                                Cancel
                            </Button>
                            <Button onClick={editor.submit} isLoading={busy}>
                                Restore
                            </Button>
                        </>
                    }
                >
                    <div className="space-y-4">
                        <div className="rounded-xl border border-border bg-surface-2/60 px-4 py-3">
                            <p className="mb-1 text-xs font-medium uppercase tracking-wide text-fg-subtle">
                                Restore to
                            </p>
                            <p className="break-all text-sm text-fg">{state.entry.historic_url}</p>
                        </div>
                        {error && <Alert tone="error">{error}</Alert>}
                    </div>
                </Dialog>
            );

        default:
            return assertNever(state);
    }
};
