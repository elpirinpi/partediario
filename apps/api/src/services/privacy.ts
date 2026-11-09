const RE_CEDULA = /\b\d{3}[- ]?\d{7}[- ]?\d\b/g;
const RE_TELEFONO = /\b(?:\+?1[\s.-]?)?(?:809|829|849)[\s.-]?\d{3}[\s.-]?\d{4}\b/g;
const RE_EMAIL = /\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}\b/g;
const RE_NOMBRE =
  /\b(nombrado|nombrada|señor|señora|ciudadano|ciudadana|propietario|propietaria)\s+([A-ZÁÉÍÓÚÑ][a-záéíóúñ]+(?:\s+(?:de\s+|del\s+)?[A-ZÁÉÍÓÚÑ][a-záéíóúñ]+){0,3})/gi;

/**
 * Sustituye datos sensibles por tokens antes de enviar texto a la IA y los
 * restaura en la respuesta. Los originales solo viven en memoria.
 */
export class Anonymizer {
  private replacements: Array<[token: string, original: string]> = [];

  anonymize(text: string): string {
    let out = this.replaceNamed(text);
    out = this.replaceRegex(out, RE_CEDULA, "CEDULA");
    out = this.replaceRegex(out, RE_TELEFONO, "TELEFONO");
    out = this.replaceRegex(out, RE_EMAIL, "CORREO");
    return out;
  }

  restore(text: string): string {
    let out = text;
    for (const [token, original] of this.replacements) {
      out = out.split(token).join(original);
    }
    return out;
  }

  private token(kind: string, original: string): string {
    const existing = this.replacements.find(([, value]) => value === original);
    if (existing) return existing[0];
    const token = `<<${kind}_${this.replacements.length + 1}>>`;
    this.replacements.push([token, original]);
    return token;
  }

  private replaceRegex(text: string, re: RegExp, kind: string): string {
    const global = new RegExp(re.source, re.flags.includes("g") ? re.flags : `${re.flags}g`);
    return text.replace(global, (match) => this.token(kind, match));
  }

  private replaceNamed(text: string): string {
    const global = new RegExp(RE_NOMBRE.source, "gi");
    return text.replace(global, (_full, prefix: string, name: string) => {
      return `${prefix} ${this.token("NOMBRE", name)}`;
    });
  }
}
