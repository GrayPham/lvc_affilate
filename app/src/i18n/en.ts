// ENGLISH dictionary. Typed `Record<DictKey, string>` so a missing key breaks the build.
import type { DictKey } from "./vi";

export const en: Record<DictKey, string> = {
  "app.name": "LVC Shop",
  "app.tagline": "Sales video studio",
  "theme.toggle": "Toggle light / dark",

  "gate.title": "Activate LVC Shop",
  "gate.close": "Close",
  "gate.checking": "Checking your licence…",
  "gate.step.login": "Sign in",
  "gate.step.activate": "Activate",
  "gate.login.subtitle": "Sign in to your LVC account to continue",
  "gate.login.username": "Username",
  "gate.login.password": "Password",
  "gate.login.submit": "Sign in",
  "gate.login.submitting": "Signing in…",
  "gate.login.remember": "Remember me",
  "gate.login.rememberHint": "Leave it off and you will sign in again next time.",
  "gate.password.show": "Show password",
  "gate.password.hide": "Hide password",

  "gate.activate.subtitle": "Enter your licence key to activate",
  "gate.activate.key": "Licence key",
  "gate.activate.placeholder": "XXXX-XXXX-XXXX-XXXX",
  "gate.activate.submit": "Activate",
  "gate.activate.submitting": "Activating…",
  "gate.activate.hello": "Hello {ten}",
  "gate.back": "Back to sign in",

  "gate.exit": "Quit",
  "gate.logout": "Sign out",
  "gate.buy": "Buy a licence",
  "gate.contact": "Need help?",
  "gate.register.prompt": "Don't have an account?",
  "gate.register.link": "Create a free account",
  "legal.terms": "Terms",
  "legal.privacy": "Privacy",
  "legal.support": "Support",
  "legal.version": "Version {v}",

  "gate.device.label": "This device ID",
  "gate.device.unknown": "not available",

  "gate.err.NETWORK": "Cannot reach the server. Check your connection and try again.",
  "gate.err.BAD_CREDENTIALS": "Wrong username or password.",
  "gate.err.VERIFY_FAILED": "That licence key is invalid or already used on another device.",
  "gate.err.NOT_LOGGED_IN": "Your sign-in has expired, please sign in again.",
  "gate.err.SESSION_SAVE": "Could not save the session to this device.",
  "gate.err.unknown": "Something went wrong, please try again.",
  "gate.err.noBridge": "This is a browser preview with no connection to the app, so signing in is not possible here. Open the real LVC Shop app.",
  "gate.err.notActivated": "Activation failed, please check the key.",

  "lang.switch": "Language",
};
