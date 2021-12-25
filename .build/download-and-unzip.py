import urllib.request, zipfile, sys
resp = urllib.request.urlopen(sys.argv[1])
with open("sdl-devel.zip", "wb") as f:
    f.write(resp.read())
with zipfile.ZipFile("sdl-devel.zip") as z:
    z.extractall(path=".")
