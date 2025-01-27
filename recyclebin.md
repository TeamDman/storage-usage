```pwsh
# in an elevated prompt, at the root of each disk
rm -Recurse -force '.\$RECYCLE.BIN\'
# even after emptying my recycle bin I had to run this to free up hundreds of GB of space
```