# Passerelle Scriba -> Grammalecte (grammalecte.net, GPL 3).
# python -I -u scriba_gc.py <dossier du paquet Grammalecte>
# Une ligne JSON (le texte) en entrée, une ligne JSON en sortie : [[début, fin, type, [suggestions], message], ...]
import json
import sys

sys.stdout.reconfigure(encoding="utf-8")
sys.stdin.reconfigure(encoding="utf-8")
sys.path.insert(0, sys.argv[1])

import grammalecte  # noqa: E402

checker = grammalecte.GrammarChecker("fr")
# la typographie (espaces insécables, apostrophes courbes, majuscules…) n'intéresse pas un élève qui écrit
checker.gce.setOptions({"typo": False, "apos": False, "esp": False, "nbsp": False, "maj": False, "num": False,
                        "unit": False, "virg": False, "poncfin": False, "eepi": False, "minis": False, "tu": False,
                        "nf": False, "chim": False, "date": False, "latex": False, "md": False, "pleo": False,
                        "bs": False})
checker.getParagraphErrors("Les enfant mange des pomme.", bDebug=False, bSpellSugg=False)   # échauffement
print("ready", flush=True)

for line in sys.stdin:
    try:
        text = json.loads(line)
        errs, _spell = checker.getParagraphErrors(text, bDebug=False, bSpellSugg=False)
        out = [[e["nStart"], e["nEnd"], e["sType"], list(e["aSuggestions"])[:4], e["sMessage"]] for e in errs]
    except Exception:
        out = []
    print(json.dumps(out, ensure_ascii=False), flush=True)
