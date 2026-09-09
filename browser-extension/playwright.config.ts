import {defineConfig} from '@playwright/test';
export default defineConfig({testDir:'./browser-tests',outputDir:'./test-results',workers:1,reporter:'list',use:{headless:true,screenshot:'off',video:'off',trace:'off'},projects:[{name:'chrome',use:{channel:'chrome'}},{name:'edge',use:{channel:'msedge'}}]});
