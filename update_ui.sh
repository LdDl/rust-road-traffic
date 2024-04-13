wget https://github.com/LdDl/rust-road-traffic-ui/releases/latest/download/dist.zip -O dist.zip
rm -rf src/rest_api/static
unzip dist.zip -d src/rest_api/static
rm dist.zip
