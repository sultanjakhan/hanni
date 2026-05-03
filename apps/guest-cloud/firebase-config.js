// firebase-config.js — public Firebase Web SDK config for the cloud guest UI.
//
// These values are PUBLIC (Web API key, project ID). Security is enforced by
// Firestore rules: only share_links/<token>/** is readable, and the token in
// the URL acts as the secret. See firebase/firestore.rules.
window.HANNI_FIREBASE_CONFIG = {
  apiKey: "AIzaSyDar36Hf3xoJo9-hgFo6pHdE_uwrHe0fPM",
  authDomain: "hanni-2e5d0.firebaseapp.com",
  projectId: "hanni-2e5d0",
  storageBucket: "hanni-2e5d0.firebasestorage.app",
  messagingSenderId: "839796188414",
  appId: "1:839796188414:web:d90a718df34ac7402531af",
};
