/**
 * Utility functions for score color coding in search results.
 *
 * Score thresholds:
 * - High score (≥ 0.8): Green
 * - Medium score (0.5 - 0.79): Yellow/Amber
 * - Low score (< 0.5): Red/Orange
 */

type ScoreLevel = 'high' | 'medium' | 'low';

/**
 * Get the score level based on the numeric score
 */
function getScoreLevel(score: number): ScoreLevel {
	if (score >= 0.8) return 'high';
	if (score >= 0.5) return 'medium';
	return 'low';
}

/**
 * Get the badge color class for a score (for score badges)
 */
export function getScoreBadgeClass(score: number): string {
	const level = getScoreLevel(score);
	switch (level) {
		case 'high':
			return 'bg-green-100 dark:bg-green-900/50 text-green-800 dark:text-green-200';
		case 'medium':
			return 'bg-yellow-100 dark:bg-yellow-900/50 text-yellow-800 dark:text-yellow-200';
		case 'low':
			return 'bg-red-100 dark:bg-red-900/50 text-red-800 dark:text-red-200';
	}
}

/**
 * Get the border color class for a score
 */
export function getScoreBorderClass(score: number): string {
	const level = getScoreLevel(score);
	switch (level) {
		case 'high':
			return 'border-green-300 dark:border-green-700';
		case 'medium':
			return 'border-yellow-300 dark:border-yellow-700';
		case 'low':
			return 'border-red-300 dark:border-red-700';
	}
}
