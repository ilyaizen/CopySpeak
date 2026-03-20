import { defineConfig } from 'vitest/config';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import { svelteTesting } from '@testing-library/svelte/vite';
import path from 'node:path';
import { mockAppStatePlugin } from './vitest-plugin-mock-state';

export default defineConfig({
	plugins: [mockAppStatePlugin(), svelte({ hot: !process.env.VITEST, compilerOptions: { css: 'injected' } }), svelteTesting()],
	test: {
		include: ['src/**/*.{test,spec}.{js,ts}'],
		environment: 'jsdom',
		setupFiles: ['./src/test-setup.ts'],
		globals: true,
		ssr: {
			noExternal: true,
		},
	},
	server: {
		deps: {
			inline: ['svelte', '@lucide/svelte'],
		},
	},
	resolve: {
		alias: {
			'$lib': path.resolve('./src/lib'),
			'$lib/*': path.resolve('./src/lib/*'),
		},
	},
});
