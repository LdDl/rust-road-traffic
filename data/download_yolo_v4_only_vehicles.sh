# weights (it was trained on COCO dataset for 'vehicles' class only: car, motorbike, bus, train, truck) for input size 416x256
weights_fileid="1_NNRyXO1r-FjDmJ_q9bqo_2TpVsK0n13"
weights_filename="yolov4-tiny-vehicles-rect_best.weights"
html=`curl -c ./cookie -s -L "https://drive.google.com/uc?export=download&id=${fiweights_fileidleid}"`
curl -Lb ./cookie "https://drive.google.com/uc?export=download&`echo ${html}|grep -Po '(confirm=[a-zA-Z0-9\-_]+)'`&id=${weights_fileid}" -o ${weights_filename}
rm -rf ./cookie
# You can use 'wget' as alternative:
# wget --load-cookies /tmp/cookies.txt "https://docs.google.com/uc?export=download&confirm=$(wget --quiet --save-cookies /tmp/cookies.txt --keep-session-cookies --no-check-certificate 'https://docs.google.com/uc?export=download&id=1_NNRyXO1r-FjDmJ_q9bqo_2TpVsK0n13' -O- | sed -rn 's/.*confirm=([0-9A-Za-z_]+).*/\1\n/p')&id=1_NNRyXO1r-FjDmJ_q9bqo_2TpVsK0n13" -O yolov4-tiny-vehicles-rect_best.weights && rm -rf /tmp/cookies.txt

# configuration for inference (416x256)
cfg_fileid="10L8mfn8oGLZJmqSxNtGg42bYD0QCkQAv"
cfg_filename="yolov4-tiny-vehicles-rect.cfg"
html=`curl -c ./cookie -s -L "https://drive.google.com/uc?export=download&id=${cfg_fileid}"`
curl -Lb ./cookie "https://drive.google.com/uc?export=download&`echo ${html}|grep -Po '(confirm=[a-zA-Z0-9\-_]+)'`&id=${cfg_fileid}" -o ${cfg_filename}
rm -rf ./cookie
# You can use 'wget' as alternative:
# wget --load-cookies /tmp/cookies.txt "https://docs.google.com/uc?export=download&confirm=$(wget --quiet --save-cookies /tmp/cookies.txt --keep-session-cookies --no-check-certificate 'https://docs.google.com/uc?export=download&id=10L8mfn8oGLZJmqSxNtGg42bYD0QCkQAv' -O- | sed -rn 's/.*confirm=([0-9A-Za-z_]+).*/\1\n/p')&id=10L8mfn8oGLZJmqSxNtGg42bYD0QCkQAv" -O yolov4-tiny-vehicles-rect.cfg && rm -rf /tmp/cookies.txt
