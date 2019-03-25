# Magic Leap demo

First, install the Magic Leap SDK. By default this is installed in `MagicLeap/mlsdk/<version>`, for example:
```
  export MAGICLEAP_SDK=~/MagicLeap/mlsdk/v0.19.0
```
  You will also need a signing certificate.
```
  export MLCERT=~/MagicLeap/cert/mycert.cert
```

Now build the pathfilder demo library:
```
  CFLAGS="-I${MAGICLEAP_SDK}/lumin/usr/include --sysroot=${MAGICLEAP_SDK}/lumin/usr" \
  PATH=$PATH:${MAGICLEAP_SDK}/tools/toolchains/bin/ \
  cargo build --release --target=aarch64-linux-android
```

Then build the `.mpk` archive:
```
  ${MAGICLEAP_SDK}/mabu PathfinderDemo.package -t release_device -s ${MLCERT}
```
The `.mpk` can be installed:
```
  ${MAGICLEAP_SDK}/tools/mldb/mldb install -u .out/PathfinderDemo/PathfinderDemo.mpk
```
and run:
```
  ${MAGICLEAP_SDK}/tools/mldb/mldb launch -w com.mozilla.pathfinder.demo 
```
