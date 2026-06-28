import React from 'react';
import { LogOut, ShieldCheck, UserRound } from 'lucide-react';
import { useAuth } from '../../hooks/useAuth';
import { cn } from '../../lib/cn';
import { Badge } from '../ui/Badge';
import { Button } from '../ui/Button';
import { ThemeToggle } from '../ui/ThemeToggle';
import { Logo } from './Logo';

export interface AppHeaderProps {
    /** Optional page-specific actions rendered before the user controls. */
    actions?: React.ReactNode;
    className?: string;
}

export const AppHeader: React.FC<AppHeaderProps> = ({ actions, className }) => {
    const { userInfo, authMode, logout } = useAuth();
    const userId = userInfo?.user_id;

    return (
        <header
            className={cn(
                'sticky top-0 z-30 border-b border-border bg-bg/80 backdrop-blur-md',
                className,
            )}
        >
            <div className="mx-auto flex h-14 max-w-6xl items-center justify-between gap-2.5 px-3 sm:h-16 sm:gap-3 sm:px-6">
                <Logo />

                <div className="flex items-center gap-2 sm:gap-3">
                    {actions}

                    {userInfo && (
                        <div className="hidden items-center gap-2 rounded-full border border-border bg-surface py-1 pl-1 pr-3 sm:flex">
                            <span className="flex h-7 w-7 items-center justify-center rounded-full bg-primary-soft text-primary-soft-fg">
                                {userInfo.is_admin ? (
                                    <ShieldCheck className="h-4 w-4" />
                                ) : (
                                    <UserRound className="h-4 w-4" />
                                )}
                            </span>
                            <span className="max-w-48 truncate text-sm font-medium text-fg">
                                {userId || 'Anonymous'}
                            </span>
                            {userInfo.is_admin && (
                                <Badge tone="primary" className="ml-0.5">
                                    Admin
                                </Badge>
                            )}
                        </div>
                    )}

                    <ThemeToggle />

                    {authMode === 'oauth' && (
                        <Button
                            variant="ghost"
                            size="sm"
                            onClick={logout}
                            leftIcon={<LogOut className="h-4 w-4" />}
                            aria-label="Log out"
                        >
                            <span className="hidden sm:inline">Log out</span>
                        </Button>
                    )}
                </div>
            </div>
        </header>
    );
};
