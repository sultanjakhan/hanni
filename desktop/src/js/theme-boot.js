// Run before stylesheets load so the saved theme is applied to the first paint.
document.documentElement.setAttribute(
  'data-theme',
  localStorage.getItem('hanni_theme') || 'light'
);
