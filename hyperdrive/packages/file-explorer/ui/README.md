# Hyperware UI Template

Based on the Vite React Typescript template.

## Setup

When using `kit new`, the `BASE_URL` on line 9 of `vite.config.ts` will be set automatically.
The `BASE_URL` will be the first process in `manifest.json`, the `package` from `metadata.json`, and `publisher` from `metadata.json`.
If you have multiple processes in `manifest.json`, make sure the first process will be the one serving the UI.

## Development

Run `npm i` and then `npm run dev` to start working on the UI.

You may see an error:

```
[vite] Pre-transform error: Failed to load url /our.js (resolved id: /our.js). Does the file exist?
```

You can safely ignore this error. The file will be served by the node via the proxy.

## public vs assets

The `public/assets` folder contains files that are referenced in `index.html`, `src/assets` is for asset files that are only referenced in `src` code.

## About Vite + React

This template provides a minimal setup to get React working in Vite with HMR and some ESLint rules.

Currently, two official plugins are available:

- [@vitejs/plugin-react](https://github.com/vitejs/vite-plugin-react/blob/main/packages/plugin-react/README.md) uses [Babel](https://babeljs.io/) for Fast Refresh
- [@vitejs/plugin-react-swc](https://github.com/vitejs/vite-plugin-react-swc) uses [SWC](https://swc.rs/) for Fast Refresh

## Expanding the ESLint configuration

If you are developing a production application, we recommend updating the configuration to enable type aware lint rules:

- Configure the top-level `parserOptions` property like this:

```js
export default {
  // other rules...
  parserOptions: {
    ecmaVersion: 'latest',
    sourceType: 'module',
    project: ['./tsconfig.json', './tsconfig.node.json'],
    tsconfigRootDir: __dirname,
  },
}
```

- Replace `plugin:@typescript-eslint/recommended` to `plugin:@typescript-eslint/recommended-type-checked` or `plugin:@typescript-eslint/strict-type-checked`
- Optionally add `plugin:@typescript-eslint/stylistic-type-checked`
- Install [eslint-plugin-react](https://github.com/jsx-eslint/eslint-plugin-react) and add `plugin:react/recommended` & `plugin:react/jsx-runtime` to the `extends` list
