#   Run generateterrain
#   Usage sh smallclip.sh OUTDIR
BASE="$HOME/projects/maptools"
EXECUTABLE=$BASE/rust/target/release/generateterrain
echo Running $EXECUTABLE, writing to $1
$EXECUTABLE -c $BASE/keys/generate_credentials.txt -b -g agni --clip "(1133,1049)-(1134,1050)" --outdir $1
