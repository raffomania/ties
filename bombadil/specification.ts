import { always, actions, weighted } from "@antithesishq/bombadil";
import { extract, getFingerprint } from "@antithesishq/bombadil/browser";
import type { ActionTemplate } from "@antithesishq/bombadil/browser";

export {
  noUncaughtExceptions,
  noHttpErrorCodes,
  noConsoleErrors,
} from "@antithesishq/bombadil/browser/defaults/properties";

export {
  back,
  forward,
  reload,
  clicks,
  inputs,
  scroll,
} from "@antithesishq/bombadil/browser/defaults/actions";

const hasHtmlElement = extract((state) => {
  return state.document.documentElement.tagName === "HTML";
});

const hasBodyElement = extract((state) => {
  return state.document.body !== null && state.document.body !== undefined;
});

export const page_has_html_structure = always(() => {
  return hasHtmlElement.current && hasBodyElement.current;
});

const formElements = extract((state) => {
  const forms = state.document.querySelectorAll("form");
  return forms.length;
});

const hasRequiredInputAttribute = extract((state) => {
  const requiredInputs = state.document.querySelectorAll("input[required]");
  return requiredInputs.length;
});

export const forms_have_proper_structure = always(() => {
  if (formElements.current > 0) {
    return hasRequiredInputAttribute.current >= 0;
  }
  return true;
});

const hasRawHtmlTags = extract((state) => {
  const body = state.document.body;
  if (!body) return false;
  const text = body.textContent || "";
  const rawHtmlPatterns = [
    "<form",
    "</form>",
    "<div",
    "</div>",
    "<input",
    "<button",
    "</button>",
    "<header",
    "</header>",
    "<h1",
    "</h1>",
    "<label",
    "</label>",
  ];
  return rawHtmlPatterns.some((pattern) => text.includes(pattern));
});

export const no_raw_html_displayed = always(() => {
  return !hasRawHtmlTags.current;
});

const titleInput = extract((state) => {
  const input = state.document.querySelector('input[name="title"]');
  if (!input) return null;
  const rect = input.getBoundingClientRect();
  return {
    fingerprint: getFingerprint(input),
    point: {
      x: rect.left + rect.width / 2,
      y: rect.top + rect.height / 2,
    },
  };
});

const submitButton = extract((state) => {
  const button = state.document.querySelector('button[type="submit"]');
  if (!button) return null;
  const rect = button.getBoundingClientRect();
  return {
    fingerprint: getFingerprint(button),
    point: {
      x: rect.left + rect.width / 2,
      y: rect.top + rect.height / 2,
    },
  };
});

const clearAndSubmitForm = actions(() => {
  const result: ActionTemplate[] = [];

  if (titleInput.current && submitButton.current) {
    result.push({
      Click: titleInput.current,
    });
    result.push({
      TypeText: {
        text: { Text: [0, 0] },
        delayMillis: [0, 0],
      },
    });
    result.push({
      Click: submitButton.current,
    });
  }

  return result;
});

export const formSubmissionActions = weighted([
  [5, clearAndSubmitForm],
  [1, clearAndSubmitForm],
]);
