# Fixture 05 — React single-page app

`src/main.jsx` is the source; `app.js` is the committed build (React 19.3.1 + the app, minified, about 190 KB), so tests run without Node.js and offline.
React and React DOM are MIT-licensed; their licence notices are kept at the end of `app.js`.

Rebuild after changing `src/main.jsx` (Node.js 22+):

```sh
cd tests/fixtures/sites/05-react-spa
npm install --no-save react@19.3.1 react-dom@19.3.1 esbuild
npx esbuild src/main.jsx --bundle --minify --format=iife --jsx=automatic \
  --define:process.env.NODE_ENV='"production"' --legal-comments=eof --target=es2020 --outfile=app.js
rm -rf node_modules
```

Pages: `/` (hero, stats, features, tabs, footer) and `/pricing` (plans). `/pricing` has no file: ScrollRecord's site server answers it with `index.html`, like Netlify or Vercel do for single-page apps.
