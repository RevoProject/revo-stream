import App from '../src/lib/components/Modules/GraphicPlanner.svelte';

let app = null;
let sources = [];
let sceneResolution = '1920x1080';

window.addEventListener('message', (event) => {
  if (!event.data || typeof event.data !== 'object') return;
  if (event.data.type === 'init') {
    sources = event.data.sources || [];
    sceneResolution = event.data.sceneResolution || '1920x1080';
    if (!app) {
      app = new App({
        target: document.getElementById('app'),
        props: {
          sources,
          sceneResolution,
          onClose: () => window.close(),
          onSave: (transforms) => {
            window.opener.postMessage({ type: 'saveTransforms', transforms }, '*');
          }
        }
      });
    }
  }
});

// If parent sends data after window is already open
window.opener && window.opener.postMessage({ type: 'requestInit' }, '*');
