import { describe, expect, it } from 'vitest';
import type { UrlHistoryEntry } from '../../types';
import {
    CLOSED,
    editorBusy,
    editorError,
    editorReducer,
    type EditorState,
} from './destinationEditor';

const ENTRY: UrlHistoryEntry = {
    id: 7,
    short_code: 'promo',
    historic_url: 'https://example.com/old',
    changed_at: 1_700_000_000,
    changed_by: 'u1',
};

const opened = (): EditorState =>
    editorReducer(CLOSED, { type: 'editOpened', current: 'https://example.com/new' });

const submitting = (): EditorState => editorReducer(opened(), { type: 'submitted' });

describe('opening', () => {
    it('seeds the draft from the current destination', () => {
        const state = opened();
        expect(state).toEqual({
            tag: 'editing',
            draft: 'https://example.com/new',
            status: { tag: 'idle', error: null },
        });
    });

    it('makes the two dialogs mutually exclusive', () => {
        const restoring = editorReducer(opened(), { type: 'restoreOpened', entry: ENTRY });
        expect(restoring.tag).toBe('restoring');
    });
});

describe('editing', () => {
    it('records draft changes', () => {
        const state = editorReducer(opened(), { type: 'draftChanged', draft: 'https://x' });
        expect(state).toMatchObject({ tag: 'editing', draft: 'https://x' });
    });

    it('reports a client-side rejection without leaving the dialog', () => {
        // Reachable from `idle`: an empty destination is refused before any request.
        const state = editorReducer(opened(), {
            type: 'failed',
            message: 'Destination cannot be empty',
        });
        expect(editorError(state)).toBe('Destination cannot be empty');
        expect(editorBusy(state)).toBe(false);
        expect(state.tag).toBe('editing');
    });
});

describe('while a submission is in flight', () => {
    it('is busy', () => {
        expect(editorBusy(submitting())).toBe(true);
        expect(editorError(submitting())).toBeNull();
    });

    it('cannot be dismissed', () => {
        // Closing would strand a request whose result has nowhere to land.
        const state = submitting();
        expect(editorReducer(state, { type: 'dismissed' })).toBe(state);
    });

    it('cannot be swapped for the other dialog or edited', () => {
        const state = submitting();
        expect(editorReducer(state, { type: 'restoreOpened', entry: ENTRY })).toBe(state);
        expect(editorReducer(state, { type: 'editOpened', current: 'x' })).toBe(state);
        expect(editorReducer(state, { type: 'draftChanged', draft: 'x' })).toBe(state);
    });

    it('cannot be submitted twice', () => {
        const state = submitting();
        expect(editorReducer(state, { type: 'submitted' })).toBe(state);
    });
});

describe('settling', () => {
    it('closes on success', () => {
        expect(editorReducer(submitting(), { type: 'succeeded' })).toBe(CLOSED);
    });

    it('returns to the dialog with the failure on error', () => {
        const state = editorReducer(submitting(), { type: 'failed', message: 'Rejected' });
        expect(state.tag).toBe('editing');
        expect(editorBusy(state)).toBe(false);
        expect(editorError(state)).toBe('Rejected');
    });

    it('restores from the entry carried by the state', () => {
        const state = editorReducer(
            editorReducer(CLOSED, { type: 'restoreOpened', entry: ENTRY }),
            { type: 'submitted' },
        );
        expect(state).toEqual({ tag: 'restoring', entry: ENTRY, status: { tag: 'submitting' } });
        expect(editorReducer(state, { type: 'succeeded' })).toBe(CLOSED);
    });

    it('ignores a settlement that arrives after the dialog closed', () => {
        expect(editorReducer(CLOSED, { type: 'succeeded' })).toBe(CLOSED);
        expect(editorReducer(CLOSED, { type: 'failed', message: 'late' })).toBe(CLOSED);
        expect(editorReducer(CLOSED, { type: 'draftChanged', draft: 'x' })).toBe(CLOSED);
    });
});
