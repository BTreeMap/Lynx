import { useContext } from 'react';
import { AuthContext, type AuthContextValue } from '../contexts/AuthContext';

/**
 * Access the authentication machine.
 *
 * Throwing outside a provider keeps the return type total: every consumer receives a
 * value, so none has to defend against `undefined` on a code path that only a
 * misassembled tree could reach.
 */
export const useAuth = (): AuthContextValue => {
    const context = useContext(AuthContext);
    if (context === undefined) {
        throw new Error('useAuth must be used within an AuthProvider');
    }
    return context;
};
