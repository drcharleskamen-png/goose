import React, { Suspense, lazy } from 'react';
import ReactDOM from 'react-dom/client';
import { IntlProvider } from 'react-intl';
import { ConfigProvider } from './components/ConfigContext';
import { ErrorBoundary } from './components/ErrorBoundary';
import SuspenseLoader from './suspense-loader';
import { client } from './api/client.gen';
import { applyThemeTokens } from './theme/theme-tokens';
import { currentLocale, currentMessageLocale, loadMessages } from './i18n';

// Apply theme tokens to :root before first paint.
applyThemeTokens();

const App = lazy(() => import('./App'));

let warnedFallbackLocale = false;
function handleIntlError(err: { code: string; message?: string }) {
  if (err.code === 'MISSING_TRANSLATION' && currentLocale !== currentMessageLocale) {
    if (!warnedFallbackLocale) {
      warnedFallbackLocale = true;
      console.warn(
        `[i18n] Locale "${currentLocale}" has no translations; falling back to "${currentMessageLocale}".`
      );
    }
    return;
  }
  console.error(err);
}

(async () => {
  // Check if we're in the launcher view (doesn't need goosed connection)
  const isLauncher = window.location.hash === '#/launcher';

  if (!isLauncher) {
    const backendAcpOnly = window.appConfig.get('GOOSE_DESKTOP_BACKEND') === 'acp';
    if (!backendAcpOnly) {
      const gooseApiHost = await window.electron.getGoosedHostPort();
      if (gooseApiHost === null) {
        window.alert('failed to start goose backend process');
        return;
      }
      client.setConfig({
        baseUrl: gooseApiHost,
        headers: {
          'Content-Type': 'application/json',
          'X-Secret-Key': await window.electron.getSecretKey(),
        },
      });
    }
  }

  const messages = await loadMessages(currentMessageLocale);

  ReactDOM.createRoot(document.getElementById('root')!).render(
    <React.StrictMode>
      <IntlProvider
        locale={currentLocale}
        defaultLocale="en"
        messages={messages}
        onError={handleIntlError}
      >
        <Suspense fallback={SuspenseLoader()}>
          <ConfigProvider>
            <ErrorBoundary>
              <App />
            </ErrorBoundary>
          </ConfigProvider>
        </Suspense>
      </IntlProvider>
    </React.StrictMode>
  );
})();
