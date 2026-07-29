import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { BrowserRouter } from 'react-router-dom';
import './index.css';
import App from './App.tsx';
import { ThemeProvider } from './components/ThemeProvider';

/**
 * The mount point is a precondition of the whole app, so it is checked once here rather
 * than asserted away with `!`. If `index.html` ever loses the element, this reports
 * what is wrong instead of failing inside React with a null receiver.
 */
const container = document.getElementById('root');
if (container === null) {
    throw new Error('Missing #root element: index.html and main.tsx have diverged.');
}

createRoot(container).render(
    <StrictMode>
        <ThemeProvider>
            <BrowserRouter>
                <App />
            </BrowserRouter>
        </ThemeProvider>
    </StrictMode>,
);
