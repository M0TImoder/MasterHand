import cv2
import mediapipe as mp
import socket
import json

mp_hands = mp.solutions.hands
hands = mp_hands.Hands(
    max_num_hands=2,
    min_detection_confidence=0.6,
    min_tracking_confidence=0.8
)
mp_drawing = mp.solutions.drawing_utils

sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
server_address = ('127.0.0.1', 5005)

cap = cv2.VideoCapture(0)

while cap.isOpened():
    success, image = cap.read()
    if not success:
        continue

    image = cv2.flip(image, 1)
    image_rgb = cv2.cvtColor(image, cv2.COLOR_BGR2RGB)
    results = hands.process(image_rgb)

    hand_data_list = []

    if results.multi_hand_landmarks:
        for i, hand_landmarks in enumerate(results.multi_hand_landmarks):
            
            # 左右の判定
            handedness = results.multi_handedness[i].classification[0].label
            
            landmark_list = []
            for id, lm in enumerate(hand_landmarks.landmark):
                landmark_list.append({
                    'id': id,
                    'x': lm.x,
                    'y': lm.y,
                    'z': lm.z
                })
            
            # 1つの手のデータを辞書にする
            hand_data_list.append({
                'label': handedness,
                'landmarks': landmark_list
            })
            
            # 画面描画
            mp_drawing.draw_landmarks(
                image,
                hand_landmarks,
                mp_hands.HAND_CONNECTIONS
            )

    # 手が見つかったらまとめて送信
    if hand_data_list:
        # 構造: { "hands": [ {右手データ}, {左手データ} ] }
        data = json.dumps({'hands': hand_data_list})
        sock.sendto(data.encode('utf-8'), server_address)

    cv2.imshow('MasterHand Vision', image)
    if cv2.waitKey(5) & 0xFF == 27:
        break

cap.release()
cv2.destroyAllWindows()
