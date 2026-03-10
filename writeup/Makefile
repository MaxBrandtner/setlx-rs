.PHONY: build clean open

MAIN = main.tex
OUTDIR = build
JOBNAME = writeup
OUTPDF = writeup.pdf

build:
	mkdir -p $(OUTDIR)/chapters
	mkdir -p $(OUTDIR)/appendix
	latexmk -pdf -outdir=$(OUTDIR) -jobname=$(JOBNAME) $(MAIN)
	makeglossaries -d $(OUTDIR) $(JOBNAME)
	latexmk -pdf -outdir=$(OUTDIR) -jobname=$(JOBNAME) $(MAIN)
	cp $(OUTDIR)/$(JOBNAME).pdf $(OUTPDF)

clean:
	rm -rf $(OUTDIR)
open:
	xdg-open $(OUTPDF)
