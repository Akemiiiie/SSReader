import { render } from 'preact';
import { App } from './app/App';
import { AnalysisPopup } from './features/reader/AnalysisPopup';
import './styles.css';
import './features/reader/floating.css';

render(new URLSearchParams(window.location.search).get('window') === 'analysis' ? <AnalysisPopup /> : <App />, document.getElementById('app')!);
