import { describe, expect, it } from 'vitest';
import {
    failure,
    foldRemote,
    IDLE,
    LOADING,
    recover,
    success,
    valueOf,
    type RemoteData,
} from './remoteData';

const describeState = (state: RemoteData<number>): string =>
    foldRemote(state, {
        onIdle: () => 'idle',
        onLoading: () => 'loading',
        onSuccess: (value) => `success:${value}`,
        onFailure: (message) => `failure:${message}`,
    });

describe('RemoteData', () => {
    it('eliminates every variant', () => {
        expect(describeState(IDLE)).toBe('idle');
        expect(describeState(LOADING)).toBe('loading');
        expect(describeState(success(7))).toBe('success:7');
        expect(describeState(failure('boom'))).toBe('failure:boom');
    });

    it('projects only success to a value', () => {
        expect(valueOf(success(7))).toBe(7);
        expect(valueOf(IDLE)).toBeUndefined();
        expect(valueOf(LOADING)).toBeUndefined();
        expect(valueOf(failure('boom'))).toBeUndefined();
    });

    describe('recover', () => {
        const fallback = success(0);

        it('substitutes the fallback for a failure', () => {
            expect(recover(failure('boom'), fallback)).toBe(fallback);
        });

        it('leaves every other variant alone', () => {
            // In particular `loading` must not be recovered: a request in flight is not
            // an absence of data, and treating it as one would flash an empty state.
            expect(recover(LOADING, fallback)).toBe(LOADING);
            expect(recover(IDLE, fallback)).toBe(IDLE);
            const value = success(42);
            expect(recover(value, fallback)).toBe(value);
        });

        it('preserves the fallback identity, so memoised consumers do not rerun', () => {
            expect(recover(failure('a'), fallback)).toBe(recover(failure('b'), fallback));
        });
    });
});
