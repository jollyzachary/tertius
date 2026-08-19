import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import App from './App';
import Overlay from './Overlay';
import './styles.css';

const overlay = new URLSearchParams(window.location.search).has('overlay');
if (overlay) document.documentElement.classList.add('overlay-document');

createRoot(document.getElementById('root')!).render(
  <StrictMode>{overlay ? <Overlay /> : <App />}</StrictMode>,
);
