export type Theme = 'light' | 'dark' | 'system';

const THEME_STORAGE_KEY = 'theme-preference';

export function getTheme(): Theme {
	if (typeof localStorage === 'undefined') {
		return 'system';
	}
	return (localStorage.getItem(THEME_STORAGE_KEY) as Theme) || 'system';
}

export function setTheme(theme: Theme): void {
	if (typeof localStorage === 'undefined') {
		return;
	}
	localStorage.setItem(THEME_STORAGE_KEY, theme);
	applyTheme(theme);
}

export function applyTheme(theme: Theme): void {
	const root = document.documentElement;
	const prefersDark = window.matchMedia('(prefers-color-scheme: dark)').matches;
	const isDark = theme === 'dark' || (theme === 'system' && prefersDark);

	if (isDark) {
		root.classList.add('dark');
	} else {
		root.classList.remove('dark');
	}
}

export function initializeTheme(): void {
	const theme = getTheme();
	applyTheme(theme);

	// Watch for system theme changes
	if (typeof window !== 'undefined') {
		const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');
		const handleChange = () => {
			if (getTheme() === 'system') {
				applyTheme('system');
			}
		};

		// Modern browsers
		if (mediaQuery.addEventListener) {
			mediaQuery.addEventListener('change', handleChange);
		} else if (mediaQuery.addListener) {
			// Fallback for older browsers
			mediaQuery.addListener(handleChange);
		}
	}
}
